

================================================================
# PROMPT
# file: docs/handoff/2026-11-20_spec/00_PROMPT.md
# what it is: the ask: what we need you to write
================================================================

# The ask: write Loft's final syntax, API and design spec (render, light, objects, motion)

You are Loft's chief scientist. You design; we implement; you guide. This bundle is everything you need. Please write
the **final specification** of the Loft meta engine: its language (syntax and semantics), its function and type
signatures, and its systems design, for 3D scenes, objects, lighting and motion. We will implement exactly what you
specify, phase by phase, and come back to you with measured numbers at every phase.

## What Loft is now

A **meta engine**: a small core that compiles and runs compositions; sub meta engines for **render**, **light** and
**motion**, each a set of interfaces, orchestration and library functions; and **example engines** written in one
language (`.loft`, your WGSL superset) that any person or model can read, copy, fork and rewrite. The stance is in
`PHILOSOPHY` below. The most important part of it: **highly technical code is cheap now and lock-in is expensive.**
Expose the hard parts (intersectors, BSDFs, light estimators, schedules, solvers, controllers) as ordinary rewritable
code; never seal them behind a framework. Judge a design by what the consumer can change, not by code saved.

Back-compat does not matter. The old TypeScript API and the old Rust engines are left behind (tag
`legacy/2026-11-19`); every example and seed will be regenerated in the new language.

## What we need from you

1. **The language, final.** Grammar and semantics of `.loft`: your draft (`LANGUAGE DRAFT` below) made final, with
   anything you change and why. Include modules and imports, how a library is written and forked, comptime and
   staging, kinds and rows, queries, schedules, laws, solvers, constraints and verdicts, units, spaces, smoothness,
   derived bounds, `raw wgsl`, and the diagnostics format.
2. **The standard library's signatures.** Every function and type an author or engine writer touches, grouped:
   - geometry: analytic primitives with exact intersections and their Lipschitz axioms; SDF primitives and operators;
     domain maps (mirror, repeat grid and polar, folds, kaleidoscope, spiral, warps with and without inverses);
     curves, frames and scans (parallel transport); surfaces (sweep, loft, lathe, extrude, thicken); meshes in and out;
     instancing; noise with tight bounds and smoothness tags;
   - materials and light: the open `Bsdf` algebra (lobes, layering, energy compensation), lights and emitters, sky and
     environment, media (marched and analytic), the six queries (`trace`, `visible`, `material`, `light`, `sky`,
     `medium`) as interfaces, the light estimators (direct, many-light, shadows from every geometry kind, GI,
     reflections, temporal reuse) and the reference path tracer;
   - non-PBR looks: how a toon, ink, woodblock, painterly or neon look is written as a composition of the same pieces,
     replacing any lobe, the shade step, the light fold, or the whole schedule;
   - camera and post: exposure, tone maps, depth of field, motion blur, bloom, grain, grading, outlines;
   - motion: bodies, ends and pins, laws, solvers, constraints, controllers, clips and retargeting, gaze, stepping,
     balance, springs, reactions (see `MOTION HANDOFF` below; that note lists the questions we most want answered);
   - schedules: the pass graph, rates, tiers, culling and indirect draws, what the compiler decides and what the
     author may override.
3. **The systems design.** The four layers (runner, core IR and gates, regenerable compiler passes, library and
   engines), the data model (kinds, rows, tables, state store, checkpoints, visibility record), the lowering rules
   (one module per kind, dependency-wave batching, (wave, law) bucketing, RenderBundle plus one `drawIndirect` per
   kind), Tier 0/Tier 1, and the gates.
4. **Example engines, written out**: `realtime` (vis-buffer deferred plus forward translucency plus traced
   reflections), `reference` (path tracer), one stylised engine (toon or woodblock), `roam` (Spore-like adaptive
   locomotion) and `act` (Disney-style performance). Each as `.loft` source an agent could fork.
5. **Three scenes, written in full**: the hermit crab boss (shell, claw, face, legs, antennae, wet chitin, reacting to
   a hit), a heron hunting (gaze locked on a fish while hopping; neck limit; then the body turns), and a stylised
   scene that breaks PBR entirely.
6. **The escape story, explicitly**: for each of these, what the author writes to fork it, and what they keep for
   free (depth, shadows, analysis, motion verdicts): a single BSDF lobe; the whole material model; the light
   transport; one shader of one kind; the visibility record; the schedule; a motion law; the solver; the whole engine.
7. **Predicted numbers** for the three scenes at 1280×800 on an Apple M4: shader modules, pipelines, GPU commands per
   frame, p50/p95 frame time, cold/warm load, motion cost for 1 and 1,000 creatures. We will measure and report back.
8. **The order to build it in**, as vertical slices with pass/fail checks, so each phase ends in a picture and numbers.

Read in this order: `PROMPT` (this), `STATE AND LESSONS`, `PATTERNS TODAY`, `PHILOSOPHY`, `CORE`, `META ENGINE
PLAN`, `LANGUAGE DRAFT`, `RENDER ARCHITECTURE`, `MOTION HANDOFF`, `FOLDING AND TRACING`, `REFERENCES`, then the
appendices (`LOFT CONTEXT`, `SIGNATURES`) as the record of the old API's names and coverage. Be ambitious; nothing has
to survive except the measured lessons.


================================================================
# STATE AND LESSONS
# file: docs/handoff/2026-11-20_spec/01_STATE_AND_LESSONS.md
# what it is: where Loft stands, every number that decides a design, what went well, where we turned poorly
================================================================

# State and lessons (20 Nov 2026)

## Where things stand

- **Old Loft** (tag `legacy/2026-11-19`): authors wrote TypeScript that built a hash-consed expression graph, with
  maths as WGSL text inside `wgsl` templates; a TypeScript compiler emitted artefacts; a Rust/wgpu loader or Dawn ran
  them. About 47k lines of TS in `src`, 19k of Rust. 125 examples, 170 training seeds, a creative training pipeline
  pinned to it.
- **Lean Rust port** (abandoned): a redesign and port at once; it reached 233/295 corpus scenes drawing, 13 passing
  every frame, at 30× the frame time of the old path.
- **Design slices** (on main, kept as evidence): render "kinds" drew 10, 100 and 1,000 things with the same 6 shader
  modules and pipelines (2.40 ms for 10 things, 4.60 ms for 30 at 1280×800); motion `roam` ran a live crab on Rust
  bit-equal to its bake.
- **New direction**: the meta engine in `.loft` (your draft), Phase 0 (the spine) starting now.

## Numbers that decide designs (Apple M4)

| What | Number |
|---|---|
| Five crabs, old Rust loader on TS artefacts | 3.0 / 3.5 ms p50/p95, 305 modules, 422 render + 24 compute pipelines, 32.9 s cold / 3.6 s warm load |
| Same, TS engine on Dawn | 4.3 ms |
| Same, redesigned Rust path | 97 / 109 ms, 5,539 modules, 4,905 compute pipelines, 23 s load |
| Pose stages | level-batched 24 dispatches 0.83 ms vs per-kernel 4,884 dispatches 13.7 ms |
| Draws | shader sharing 2.2 ms vs one pipeline per draw 17.3 ms |
| Frame-time model across 229 scenes | ≈ 1.3 ms + 15 µs per GPU command |
| Titan recording | 707k nodes (unrolled loft, constants counted twice, every call inlined) → 308k after fixes |
| Tape interpreter in one WGSL shader | +2.0 ms (63 ops), +5.5 ms (larger) per frame |
| Analytic branch list vs bricked march (3,069-branch coral, 1280×800) | 3.08 vs 25.89 ms close, 0.72 vs 1.90 ms far; compile 7.2 s → 31 ms; 1.5–9.5% of pixels changed |
| 100,000-grain veil | 120.8 ms marching per grain pixel vs 1.56 ms grain draw with a 32³ grid |
| Motion, 1 / 256 / 1,000 crabs fused | 0.070 / 0.101 / 0.086 ms p95; bake 5.3 ms; 216 tokens of author code |
| Rust emit with naga validation | most of emit time; validation done twice (ours and wgpu's) |

## What went well

- **Writing maths as WGSL text** inside the graph: models write it fluently, and the CPU twin, checks and solve still
  read it. Raw functions as the escape hatch, named opaque.
- **Kinds as the unit of code identity** (design slice): module count independent of instance count, proven at 1,000
  things.
- **Level batching and shader sharing** in the old compiler: they are why the old path was fast.
- **Honest checks**: interval proofs, `Unproved`/`Undecided` verdicts, refusals that name the fix; a strict parity
  runner once it was made strict.
- **Motion results**: solve near hand keys plus world-space gaze lock and neck IK (the heron the owner liked);
  weight-driven stepping from capture point (the crab that does not moonwalk); live equals bake to the bit.
- **Escape ladder**: graph function → raw WGSL function → `draw.shader` → compute pass at build/load/frame/steps.
- **PBR as swappable parts**: `pbr(parts)` takes any of distribution, visibility, fresnel, diffuse, environment,
  occlusion, sky, tone, lobe albedo; a swapped lobe re-integrates its environment albedo.
- **Keel-style devex**: compile locks, per-worktree targets, lane scripts, one full suite per wave.

## Where we turned poorly

1. **Redesign and port at once.** New IR, new wire, new unified types and a new runtime together; nothing ran end to
   end until the last lane; the speed tricks of the old compiler were lost.
2. **Lanes cut by layer**, each passing a unit check, integrating late: two incompatible wire formats; zero draws
   recorded until wave 5.
3. **Host tracing without staging**: JS loops unrolled, calls inlined, instance numbers baked into code: the root of
   the node, module and cold-compile blowups.
4. **Optimised CPU build time** (recorder, solve) before measuring GPU frame time, which is what matters.
5. **Measured frame time last.** The 97 ms result arrived after a day of work.
6. **A weak gate**: picture parity exited 0 with missing frames.
7. **Frozen layers inside the "open" design**: the material model reached the renderer as one paint struct read by
   one style; light transport welded to the style's shade function; volumes composited in author order (red over blue
   gives a different answer from blue over red, both wrong); ambient occlusion that depends on the drawing method.
8. **Shadows were stand-ins**: casts from capsule skeletons and fitted grids, sometimes sampled at time zero; a scene
   query (`castRay`) that missed things without skeletons.
9. **No asset import**, sea/sun/style mandatory in the core, about 800 public names.
10. **Solve without posture holds**: correct maths, wrong performance (the 67° heron).

## Constraints of the machine and the team

- One Apple M4 Mac, 24 GB, shared with a game studio's builds (Keel); two restarts on 19 Nov.
- Implementers: Codex gpt-6.2-sol lanes for bulk, Opus for design and integration, Astra for audits.
- Targets: native wgpu (Metal, Vulkan, DX12) and browser WebGPU; embeddable on a device a game owns (Keel,
  room-runners).
- Training uses Loft scenes written by models, scored on looks; build budget 2 s CPU, draw budget 50 ms GPU per frame
  at 512² × 16 frames.


================================================================
# PATTERNS TODAY
# file: docs/handoff/2026-11-20_spec/02_PATTERNS_TODAY.md
# what it is: what an author could do in old Loft: geometry, folding, PBR, stylised looks, escape hatches, motion
================================================================

# Patterns in old Loft: what an author could do, and how it composed

Each pattern: the API as it was, how it reached the GPU, how it met lighting, and what we would keep or change. File
paths are in the tagged tree `legacy/2026-11-19`.

## 1. Geometry: five ways to draw a thing, one depth

| Way | API | How it draws | Notes |
|---|---|---|---|
| Raster analytic surfaces | `strand`, `sweep(spine, section)`, `loft(curves)`, `lathe`, `extrude`, `thicken`, `draw.surface(name, surface, { paint, grid })` | a (u, v) grid posed in the vertex stage, painted per pixel, ink outline hull | swept bodies have analytic normals; frames along a spine are a scan (parallel transport), computed per frame by pose stages |
| Sphere-traced fields | `draw.field(name, field, opts)` | the field's bounding box rasterised; each pixel sphere-traces inside it; refinement to ~2e-4 of the analytic hit | needs a Lipschitz bound ("safety") when the field reads textures; many-part fields were N-way unions (slow) |
| Relief | `draw.relief` | shell-traced height over a surface | |
| Analytic impostors | `impostors`, `draw.copies` with sphere/capsule/cone, `draw.traced` for meshes and solids | one exact ray-primitive solve per lattice cell or listed copy | the coral test: listed analytic branches 8× faster than a bricked march |
| Meshes | `draw.mesh` | triangles | none come in; bake writes glTF out |
| Instancing | `draw.posed`, `draw.copies` (minPixels, keepLevels), `draw.points` ("specks") | instanced draws, LOD by screen size | |
| Media | `draw.density` (analytic Gaussian), `draw.volume` (marched extinction field, HG phase `g`, shadow march) | per-pixel integration up to opaque depth | each volume one composited layer in author order |

All of them write the same depth, so a field and a surface occlude each other; fields write projected hit depth.

## 2. Space folding and domain maps

`warp(shape, F)`: a `Warp` returns `{ x, id, stretch }` (mapped coordinate, the cell it came from, the local change of
length so distances stay distances). On a field or paint it is folding: mirror, repeat, polar repeat, spiral,
kaleidoscope. On a surface or curve it reparametrises. On points it maps the index (a generator of instances).
`place(shape, chart)` puts a shape in a coordinate system (on the sea, in a whirlpool, on a body); placement acts on
skeletons by default. Products: `sweep`, `sheet`, `loft`, `lathe`, `extrude`, `thicken`; "points × shape" is
instancing; "shape × time" a trail. A table is a function too: `tabulate(f, n, when)` fills at build, per frame or
never. Detail in `FOLDING AND TRACING` below.

What the draft fixes: domain maps did not create spaces, so a point in a mirrored, offset space could be used as a body
point (the gaze bug); cells were ids without a typed map back.

## 3. How fields and surfaces met PBR

There was no separate material system. A thing's `paint` (a function of its coordinates, or a struct of "words":
`base`, `gloss`, `metal`, `emits`, `rough`, `normal`) reached the style's `shade(ctx)` with a `ShadeCtx` (paint,
normal, sun, view, world point, the light fold `lights(combine, term)`, `channel(word)`, `scatter(toward)`, `sky(dir)`).
A field hit and a raster fragment produced the same `ShadeCtx`, so a traced claw and a swept leg shaded the same way.

`pbr(parts)` is a style built from swappable parts:

```ts
type PbrParts = {
  distribution: (n_h, rough) => F;                    // ggx
  visibility:   (n_l, n_v, rough) => F;               // smithGgx
  fresnel:      (v_h, f0) => P3;                       // schlick
  diffuse:      (albedo, n_l, n_v, v_h, rough) => P3;  // lambert
  environment:  (at, rough, sky) => { diffuse, specular };   // skyLight (split sum)
  occlusion:    (ctx) => F;                            // distance-field AO, or 1
  sky:          (dir, sun) => P3;                      // daylight
  tone:         (c) => P3;                             // aces
  lobeAlbedo:   (n_v, rough) => { scale, bias };       // integrated from distribution+visibility on the twin
  clear:        [r, g, b];
};
```

Lights: the sun plus up to seven `lights`, each a function of a world point returning `{ toward, amount, distance?,
size? }`; a lamp may cast depth-map shadows. A style never sees a list: it folds them.

Known flaws (from the critiques): single-scattering GGX loses energy at high roughness; the sky reflection cannot show
off-screen objects; AO differs between fields and raster; shadows on water came from capsule skeletons; volumes
composite in author order; light transport is welded to `shade`.

## 4. Breaking out of PBR (stylised looks)

A `Style` is one value: `shade`, `volume`, `ink`, `mark` (alpha for post), `water`, `shadow`, `sky`, `post` (full-screen
WGSL passes in order), `block`, `lights`. Shipped styles: `toon`, `woodblock` (with named lightings: flat, twoTone,
threeTone, bokashi, hatched, contact, rim, cel), `biolume`, `papercraft`, `sketch`, `neon`, `pbr`. Each reads only the
paint words it wants and notes the ones it ignores. `sketch` is written out in full in `docs/styles.md` as the
template for a new style. Post passes read the picture and its depth (biolume's reflection, papercraft creases).

Ways out, from smallest to largest:
1. Replace one PBR part (`pbr({ distribution: myLobe })`).
2. Write a new `Style` (any `shade`, `ink`, `post`).
3. Paint returns its own words; a style of your own reads them.
4. A raw WGSL function anywhere in the graph (`dpdx`, textures, integers): emitted as written, opaque to the twin.
5. `draw.shader(name, wgsl, { vertices, instances, phase, blend, depthWrite, cull })`: your own shader with the scene's
   camera, depth and tables.
6. `compute(name, { when: build|load|frame|steps, writes, dispatch, wgsl })`: any GPU program, outputs readable by
   everything else.
7. `--engine realtime|reference|print|inspect`: the design slice's pass lists (ordered scenes).

What was missing: forking one shader of one kind while keeping the rest; replacing the visibility record or the light
transport without rewriting a style; a stylised look that still gets correct shadows and reflections for free.

## 5. Escape hatches and seeing the code

- `wgsl` text with `${}` holes lowers a WGSL subset into the graph (loops, if, struct returns, swizzles); outside the
  subset a function is raw and named opaque; errors quote the author's line; `loft show` prints every module with
  `/* file:line */` comments.
- Holes bring params, inputs, solved curves, tables and other functions in; strings are refused.

## 6. Motion: what existed

| Piece | API | What it is |
|---|---|---|
| Curves | `keys(points, { interp })`, `bspline(t, over, values)` | keyed and B-spline curves of time |
| Gait | `gait(spec)` | a walk clock: feet lift and plant on a phase (duty ~0.66); bob, sway and roll are sines of that clock |
| Stance | `src/geom/stance.ts` | a sum type of carriages (idle, brace, dance, walk) blended by envelopes; feet planted at homes, moved only by named events (re-plants, stamps, walk) |
| IK and frames | `limb`, `chain(spec)`, `poseFrame`, `turnedAbout` | analytic three-segment limb IK; knees fold when the body moves over planted feet |
| Solve | `solve({ over, vary: { x: spline(n, from) }, hold: (u) => [always([time, a, b], equal(...)), ge(...)], prefer: { jerk \| acceleration \| near } })` | cubic B-spline unknowns; holds on position, velocity, acceleration; least jerk; augmented Lagrangian with Newton steps and exact Hessians; runs once at build; holds `Unproved` in builds, proved by `loft check` |
| Recurrences | `recur`, `iterate`, compute `steps` | stateful simulation with kept states and checkpoints |
| Weight-driven stepping (look round) | a `recur` over 8 feet at 120 steps/s | capture-point stepping from where the foot is; lean ≈ atan(a/g) |
| Gaze and neck (look round) | head placed per frame (level gaze, glance ≤ 15°), neck a fixed-length curve between shoulder and head frames | |
| New motion slice (`src/motion`) | `body({ trunk, chains: [{ root, lengths, end: foot(at) }] })`, `move(body, roam(laws?), { go: input(...), script })`, `roam({ arch?, step?, present?, deck? })` | laws are WGSL files with value-only interfaces (`seams`, `support`, `share_range`, `pin`, `deck`, `roam`, `present`); replace one law or the whole module; all creatures in one fused dispatch; live equals bake to the bit |

The new motion slice's whole crab:

```ts
import { body, foot, move, roam } from "loft/motion";
const legs = [[1.35,2.8,6.6],[0.5,1.1,7.4],[-0.4,-0.9,7.3],[-1.3,-2.7,6.4]] as const;
export const crab = body({
  trunk: { size:[4.8,1.05,5], height:2.5 },
  chains: [-1,1].flatMap(s=>legs.map(([x,fx,fz])=>({ root:[x,0.1,2.5*s], lengths:[3,2.9,1.2], end:foot([fx,0,fz*s]) }))),
});
export default stage(move(crab, roam(), { go: input("stick","vec2",[0,0]), script: stick }));
```

## 7. Mixing bespoke and physical motion (what worked)

The heron that read best: the **body** solved near the animator's keys (keys are the prior, solve adds the flight arc
and contacts), the **head** placed per frame by a gaze law in world space, the **neck** solved per frame between them.
The crab that read best: the **trunk** follows a directed path, the **feet** step from weight, the swings vary with
urgency. In both, authored intent sets the targets and physical laws fill in weight and contact. The open question for
the final design is how to make that layering a first-class, rewritable composition (see `MOTION HANDOFF`).


================================================================
# PHILOSOPHY
# file: docs/PHILOSOPHY.md
# what it is: the stance: meta engine; hard code is cheap, lock-in is expensive
================================================================

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

- **The core is tiny.** It loads a set of GPU programs, resources and an ordered list of passes and runs them on a
  device someone else may own. On the order of hundreds of lines. Everything else is library.
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

- Write maths as maths (WGSL text with holes), not as nested calls.
- A new name must buy a capability the consumer did not have.
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


================================================================
# CORE
# file: CORE.md
# what it is: priorities and definition of done
================================================================

# Loft: core

What Loft is for, what we want from it, and how we judge a change. The stance behind it is `docs/PHILOSOPHY.md`;
the design goals are `docs/design/render.md` and `docs/design/motion.md`. Everything else (DESIGN.md, the proposals, the
lane notes) serves this page. When a design and this page disagree, change the design or argue here.

## The aim

The best 3D rendering library a model (or a person) can write for: pictures that look state-of-the-art, drawn very
fast on the GPU, from short code that says exactly what is meant, with nothing the author cannot reach.

The product is the picture. The reward we train on is aesthetic; checks and `solve` are authoring tools.

## Priorities, in order

1. **The picture.** State-of-the-art looks: lighting, materials, motion, camera.
2. **GPU frame time.** Speed is judged on the GPU: frame time, then pipeline compile and load time. CPU build time is
   a cap (2 s for training under the Rust compiler), not a goal; it gets work only when the cap binds.
3. **Tokens.** The author writes the least code that says the thing, in a form they already know (maths and WGSL).
4. **Truth.** Nothing Loft reports may be false: a check, a solve, a verdict or an error. What cannot be decided says
   so by name.
5. **Lean.** Little code, few types, one path. Every name buys a capability the author did not have.

## What we want

### Drawing

- **Analytic intersection** wherever the maths solves it: impostors (sphere, capsule, cone), planes and the sea, and
  more closed forms (quadrics, boxes, tori) before anything iterative.
- **Raymarching**: sphere-traced fields inside a bound, folding and domain repetition (a `Warp` gives the cell id),
  relief on surfaces, hit refinement, normals from the field's gradient.
- **Raster analytic surfaces**: sweep, loft, lathe, extrude, thicken, evaluated in the vertex stage.
- **Instancing**: copies, posed and points, with culling and levels of detail. Many parts of one field use a spatial
  lookup (cells or bounds), never an N-way `min` at every step.
- **Media and particles**: marched and Gaussian volumes with one transmittance; specks for dust, spray and rain.
- **Physically based shading** at the state of the art: microfacet BRDF with energy compensation, image-based and sky
  light, a moving sun, good shadows and AO, subsurface where it matters. Audit against the state of the art and close
  the gaps; do not assume we are there.
- **Camera and post**: depth of field, film grain, a pass composer, multi-pass and feedback.

### Speed

- **Frame budgets**: training draws 512² × 16 frames at ≤ 50 ms GPU a frame; viewing is judged by p95 frame time.
- **Emit decides per site by GPU cost**: inline a small or once-called function, unroll a constant loop of ≤ ~8 small
  bodies, otherwise a call or a loop over a table, and a spatial lookup for many instances. The recorded graph stays
  compact either way; unrolling is an emit choice, never a recording.
- **Fewer reads, fewer registers**: packed tables, folded constants, small shaders, a pipeline cache.
- **Load time does not grow with content**: shaders are shared per kind of thing, with per-instance data in tables.
  Five crabs need a handful of shader modules, not 305 (measured 55 s to load on 19 Nov). Score shader module count
  and load time beside frame time.
- **Precision**: f32. f16 stays frozen until a scene shows a frame-time gain.

### Engine

- **One path, in Rust on wgpu**: build, serve, tools, the wasm viewer and Keel share it, on Metal, Vulkan, DX12 and
  WebGPU. TypeScript records the author's program into typed arrays and nothing more.
- **A compact graph**: a graph function is a WGSL function and its uses are calls; a repeat is a loop; instances and
  stations are tables; sibling subgraphs that differ only in constants are recorded once with a table of the
  constants. Graph size follows distinct code, not the number of uses.
- **Few types**: one kernel, one ray template, one medium, one verdict, one error, one format, one identity, one
  operator table. Kinds are sum types with one exhaustive dispatcher; inputs are classified once at the boundary; a
  missing case is a typed refusal that names the fix, never a fallback.
- **A small core, content outside it**: sea, sun and style are library values a scene may omit, not part of the core.
  Assets come in (meshes, textures, motion clips) and picking and readback go out.
- **Embeddable**: a Loft subject draws on a device someone else owns (Keel and room-runners), with wgpu and naga
  pinned to the host's versions.

### Authoring

- **WGSL text with holes** (`wgsl\`...\``) lowered into the same graph, so the CPU twin, checks and `solve` read it.
- **One escape ladder**: graph function, raw function, `draw.shader`, compute. Each rung can call the ones below it.
- **Compute at every time**: build, load, frame and steps. Any maths, any algorithm.
- **Nothing hidden**: `loft show` prints the generated WGSL; errors quote the author's line.
- **Swappable parts**: looks, materials and motions are values you replace. Libraries save tokens and never fence the
  author in.
- **Docs that cannot drift**: `loft context` is generated from the code; changes ship with a migration note and a
  codemod.

### Truth

- **Checks and `solve` never say something false**. A build solves and reports holds as `Unproved`; `loft check`
  proves; a margin finer than the arithmetic is `Undecided` by name.
- **Picture parity is the gate** for engine changes (`tools/parity`, rules in the lean Rust plan §2.2), not byte
  identity.
- **One identity record** for a picture: source, precision, parameters, inputs, format.

## How a change is judged

- **Theory first, to the number.** Predict what the run will show (frame ms, node count, parity) in the lane note
  before building; run once to score it. A miss fixes the theory.
- **Score by the priorities above**, in order: picture parity, then frame time, then tokens, then lines.
- **One full suite per wave**, run by the integrator; workers run focused checks. The machine is shared.

## Not goals

- CPU compile speed for its own sake.
- Back-compat for code. Persisted data (bundles, saved scenes) gets one reader or a migration; code changes with its
  callers.
- Two engines kept alive. Alternatives live on branches; the default is the only path once parity passes.
- Convenience names, mirrors of internals, or options that buy no capability.

## Where we are (19 Nov 2026)

| Want | State | Owner |
|---|---|---|
| Analytic impostors, raymarching, raster surfaces, instancing, media, post | Built in the TS engine | DESIGN.md |
| Rust path | Builds 233/295 corpus entries, loads and draws 233; 13 pass picture parity in every frame (full run at 963c667, before the grain fix) | lean Rust plan, `agent_notes/2026-11-19_lean_rust_draws.md` |
| Ordered render (`loft/render`) | Kinds, six queries over a world, print/realtime/reference/inspect engines, indexed indirect grids; same 6 pipelines at 10–1,000 things; 13/17 slice checks pass (cold load, one-bounce bias, time model, consumer tokens open) | `agent_notes/2026-11-19_design/render_step3_impl.md` |
| Motion (`loft/motion`) | BODY/STATE/POSE rows, one fused dispatch per frame, baked and live takes; the crab plays, seeks and draws on the Rust path; 0/481 state differences across tick groupings; owner verdict on the look pending | `agent_notes/2026-11-19_design/motion_step3_impl.md` |
| Render reads motion | Ordered scenes take row steps; the live post-IK POSE drives record and ray observers through playback, seek and rewind with zero snapshot or off-edge pixel errors. Wire-4 grid draws still bake row counts into shader text | `agent_notes/2026-11-19_design/integration.md` |
| Compact graph | Not yet: the TS engine inlines every call (Titan records 308k nodes) | switch-to-Rust lane |
| Spatial lookup for many-part fields | Not yet | — |
| State-of-the-art PBR | PBR with AO and a moving sun; gap to the state of the art not assessed | — |
| Frame time on the Rust path | Scored per entry at 960×540: median p95 6.1 ms; five-crab lineup 97/109 ms p50/p95, 5,538 modules, 23 s load | parity run |
| Checks and solve never false | Rust solve built (holds `Unproved` in builds, proofs in `loft check`); heron "moved" is a named ill-conditioned exception | `agent_notes/2026-11-19_lean_rust_solve2.md` |
| Keel embedding | loft-gpu instances and pins built; Keel side not wired | `docs/keel.md` |
| Creative training on latest Loft | Pinned at c8502c2; re-pin after the switch | creative `vendor/loft` |

Next, in order: the switch
to Rust with the compact graph (TS engine, host and old loader deleted), with grid draws reading row counts as data;
GPU work from the measured frame times; the PBR audit.


================================================================
# META ENGINE PLAN
# file: docs/proposals/2026-11-20_meta_engine_plan.md
# what it is: our plan from first principles, including our decisions on your draft (section 3)
================================================================

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
| **2. Compiler passes** (regenerable behind the gates) | Parser (WGSL superset), checker (spaces, units, smoothness), staging, forward AD, intervals and derived Lipschitz, dependency-wave batching, (wave, law) bucketing, kind dedup, cull and indirect args, naga emit, tape emit, CPU twin | ~10k lines, one module per pass | Often |
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
- **One surface, `.loft` files.** No TypeScript anywhere in the product. The CLI and the WASM build are Rust.
- **Units ship first-class but inferred**; literals need suffixes only where a unit is ambiguous.
- **Tier 0 is edit-time only**, one dirty kind at a time, kind-bucketed; motion and solver laws never run in Tier 0.
- **Bit-identity is per device**; across devices the contract is the stated tolerances.

Open, for the chief scientist (motion) and the first slice (render): see §6 and
`docs/handoff/2026-11-19_motion_chief_scientist.md`.

## 4. Gates (live from phase 0)

| Gate | Pass condition |
|---|---|
| Pictures | Every scene in the new corpus renders every expected frame; perceptual diff against its approved reference within tolerance; a missing frame fails |
| Twin | CPU twin equals GPU per device to the bit for motion; fields within the tier contract |
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


================================================================
# LANGUAGE DRAFT (yours)
# file: agent_notes/dump_manual_nov_19th_alpha_v1.md
# what it is: your previous answer: grammar, typing, staging, tiers, the hermit crab, lowering
================================================================

Both catches in the crab example were real bugs—author-written `Field[Claw, 1.35]` silently lied about `0.045 * 26 = 1.17` from the serrated teeth plus FBM, and `mirror_x(p) - Vec(...)` staying `Pt[Body]` let the exact gaze-frame bug slip through type-checking. Accepting both fixes makes the language dramatically stronger.

Below is the complete specification for the WGSL-superset language, its typing and staging rules, the Tier 0/Tier 1 contract, the rewritten hermit crab, and the lowering/portability numbers.

---

## 1. Grammar Sketch (WGSL Superset)

Everything in valid WGSL stays valid inside `fn` and `raw wgsl { ... }` blocks. The superset adds **only** the following top-level items, type constructors, and expression forms:

```ebnf
(* 1. Top-level declarations *)
item       := wgsl_item
            | "space" ident ("," ident)* ";"
            | "unit" ident ("=" unit_expr)? ";"
            | "kind" ident comptime_params? "{" row_block? kind_member* "}"
            | "scan" ident "(" params ")" "@" "schedule" "(" sched_mode ")" "->" type block
            | "solver" ident "over" ident "{" solver_member* "}"
            | "law" ident "(" params ")" "writes" ident_list block
            | "constraint" ident "(" params ")" "->" "Verdict" block
            | "schedule" ident "{" pass_stmt* "}"

comptime_params := "[" ("comptime" ident ":" discrete_type),+ "]"
row_block       := "row" "{" (ident ":" type ";")* "}"
kind_member     := "query" ("trace"|"visible"|"material"|"light"|"sky"|"medium") fn_decl
                 | fn_decl

(* 2. Type additions (bracketed so WGSL's <T> stays unambiguous) *)
type       := wgsl_type
            | ("Pt"|"Vec"|"Dir"|"Norm") "[" space_id ("," unit_expr)? "]"
            | "Xform" "[" space_id "->" space_id "]"
            | "Map" "[" space_id "->" space_id "]"
            | "Field" "[" space_id ("," smooth_k)? "]"       (* L is inferred, never authored *)
            | "Attach" "[" space_id "]" | "Target" "[" space_id "]"
            | "Table" "[" expr "," type "]" | "RowId" "[" ident "]"
            | "Bsdf" | "Verdict"

smooth_k   := "Cinf" | "C2" | "C1" | "C0" | "Cneg1"

(* 3. Expression & statement additions *)
stmt       := wgsl_stmt
            | "map" ident "=" domain_expr ";"                 (* introduces fresh space 'ident *)
            | "comptime" ("const"|"for"|"if") ...             (* Stage 0 only *)
            | "emit" ident "{" field_inits "}" ";"            (* Stage 0: appends row to kind table *)
            | "raw" "wgsl" "(" typed_bindings ")" block       (* opaque escape hatch *)

expr       := wgsl_expr
            | expr "|>" expr                                  (* domain/lobe pipeline *)
            | "diff" "(" expr "," ident ")"                   (* compiler-checked forward/rev AD *)
            | "subgrad" "(" expr "," ident ")"                (* explicit C0 non-smooth opt-in *)
            | "bound_step" "(" expr "," expr ")"              (* conservative fallback for Unbounded *)
            | "sample_c2" "(" expr "," expr ")"               (* C2 Hermite table lookup *)
            | number unit_suffix                              (* 0.18m, 28rad, 0.5s *)

```

Nothing else is added. No classes, closures, GC types, exceptions, or TypeScript syntax.

---

## 2. Typing Rules: Spaces, Units, Domain Maps, Smoothness & Derived Lipschitz

### A. Affine Spaces & Units

* `Pt[S] - Pt[S] : Vec[S]` ; `Pt[S] + Vec[S] : Pt[S]` ; `Pt[S] + Pt[S]` is a type error.
* `Xform[A -> B] * Pt[A] : Pt[B]` ; `Xform[A -> B] * Vec[A] : Vec[B]` ; `Xform[A -> B] * Norm[A] : Norm[B]` (automatically lowers to inverse-transpose $M^{-T}$).
* `inv(Xform[A -> B]) : Xform[B -> A]`.
* Dimensioned scalars unify via rational exponent vectors (`Meters / Sec`, `1 / Kg`, `Rad`). Dimensionless `Unit` tracks interval $[0, 1]$.

### B. Domain Maps & Generative Scoped Spaces

Every domain transform (`translate`, `rot_*`, `mirror_*`, `repeat_grid`, `repeat_polar`, `warp`) returns a point in a **fresh generative space** `'m` (analogous to a Rust lifetime—zero manual space declarations required):

```wgsl
map sym = mirror_x(p) |> translate(-vec3(0.42, 0.55, 1.18)m);

```

This single statement binds four compiler-tracked projections on `sym`:

* `sym : Pt['sym]` — the mapped coordinate.
* `sym.cell` — branch/cell witness (`f32` sign $\in \{-1, +1\}$ for `mirror_x`, `u32` sector for `repeat_polar`, `vec3<i32>` for `repeat_grid`).
* `sym.to_parent : Map['sym -> S]` — auto-generated inverse pipeline **parametrized by `sym.cell**`. For `mirror_x` followed by `translate(-v)`, the compiler synthesizes `x -> mirror_sign(sym.cell, x + v)` so left and right eyes lift to their true distinct `Body` positions.
* `sym.from_parent : Map[S -> 'sym]` — forward map for transforming external directions/points (like world gaze vectors) into `'sym`.

For a non-rigid **Warp** `warp(p, fwd, inv, jac_bound)`:

* If `inv` is supplied, both `to_parent` and `from_parent` exist, and `Norm['w]` transforms via $(J_{\text{fwd}}^{-1})^T$.
* If `inv` is omitted (e.g., an arbitrary noise warp), `to_parent` is **not generated**; lifting points out of `'w` is a compile error.

### C. Smoothness Lattice ($C^k$)

Every expression carries a smoothness tag in $C^\infty \sqsubset C^2 \sqsubset C^1 \sqsubset C^0 \sqsubset C^{-1}$:

* `sin`, `cos`, `exp`, `smin_exp`: $C^\infty$.
* `smootherstep` (quintic $6t^5 - 15t^4 + 10t^3$), `smin_cubic`, `sample_c2`: $C^2$.
* `smoothstep` (cubic $3t^2 - 2t^3$), `smin_poly` (quadratic), `smooth_voronoi`: $C^1$.
* `min`, `max`, `abs`, `voronoi_cone`, `sd_box`: $C^0$.
* `step`, `floor`, `fract`, `select` on runtime condition: $C^{-1}$.

**Differentiation rule:**


$$\frac{\Gamma \vdash e : (T, C^k) \quad k \ge 1}{\Gamma \vdash \text{diff}(e, x) : (\partial T, C^{k-1})}$$


Taking `diff` of a $C^0$ expression (or `diff(diff(...))` of a $C^1$ expression) is a **compile error**. To intentionally request a generalized/clamped derivative for micro-bump shading on $C^0$ geometry, the author must write `subgrad(e, x)` (which returns $C^{-1}$ and cannot be differentiated again).

### D. Compiler-Derived Lipschitz Calculus (Never Authored)

The author writes `Field[S]`, never a numeric $L$. Over the field's active bounding box $D$ (propagated via interval arithmetic $I(e) = [e_{\min}, e_{\max}]$), the compiler computes $L(e) \in \mathbb{R}_{\ge 0} \cup \{\text{Unbounded}\}$ bottom-up:

| Construct | Derived Bound $L(e)$ | Notes |
| --- | --- | --- |
| Exact SDF primitives (`sd_sphere`, `sd_box`, `sd_capsule`, `sd_torus`) | $1$ | Axiomatically verified in stdlib |
| Approx SDF (`sd_ellipsoid(p, r)`) | $\max(r) / \min(r)$ | Or tighter interval AD bound over $D$ |
| Constants / row uniforms (independent of `p`) | $0$ |  |
| Rigid map / `mirror_x` / `abs` fold | $L(f)$ | Isometric ($L_{\text{fold}} = 1$, continuous across fold plane) |
| Uniform scale $f(s \cdot p)$ vs shape scale $s \cdot f(p / s)$ | $\Vert{}s\Vert{} \cdot L(f)$ vs $L(f)$ | Compiler recognizes shape-scaling cancellation |
| Non-uniform scale $f(v \odot p)$ | $\max(\Vert{}v_x\Vert{}, \Vert{}v_y\Vert{}, \Vert{}v_z\Vert{}) \cdot L(f)$ |  |
| `repeat_grid(p, c)` / `repeat_polar(p, n)` | $L(f)$ **with auto cell-wall clamp**, or `Unbounded` | If child is proven symmetric across cell faces, $L(f)$ holds globally; otherwise the compiler emits $\min(d_{\text{child}}, d_{\text{cell\_wall}} + \epsilon)$ so rays never cross a seam in one step |
| Smooth warp $f(\phi(p))$ | $L(f) \cdot \sup_D \Vert{}J_\phi(p)\Vert{}_2$ | Bounded soundly via interval Frobenius/row-sum norm $\sqrt{\Vert{}J\Vert{}_1 \Vert{}J\Vert{}_\infty}$ |
| Linear comb $a f \pm b g$ | $\Vert{}a\Vert{} L(f) + \Vert{}b\Vert{} L(g)$ |  |
| Product $f \cdot g$ | $\Vert{}I(f)\Vert{}_\infty L(g) + \Vert{}I(g)\Vert{}_\infty L(f)$ | Requires finite intervals $I(f), I(g)$ |
| Univariate $h(f(p))$ | $\sup_{y \in I(f)} \Vert{}h'(y)\Vert{} \cdot L(f)$ | e.g., `smootherstep(a, b, x)` has $\max \Vert{}h'\Vert{} = \frac{15}{8\Vert{}b-a\Vert{}}$ |
| `min(f, g)`, `max(f, g)` | $\max(L(f), L(g))$ | Exact by Rademacher's theorem |
| `smin_poly`, `smin_cubic`, `smin_exp`, `ssub_*` (const $k$) | $\max(L(f), L(g))$ | Gradient is a convex combination $(1-w)\nabla f + w\nabla g$, $w \in [0, 1]$ |
| Smooth blend with spatially varying $k(p)$ | $\max(L(f), L(g)) + c_{\text{blend}} L(k)$ | $c_{\text{poly}} = \frac{1}{4}$, $c_{\text{cubic}} = \frac{1}{6}$ |
| `grad_noise3(p)` / `smooth_voronoi3(p)` | Pre-verified library constant $L_{\text{noise}}$ | e.g. $L_{\text{quintic3}} = 2.45$, $L_{\text{voronoi}} = 1.0$ |
| `fbm(s * p; N, lac, gain)` | $\Vert{}s\Vert{} \cdot L_{\text{noise}} \cdot \sum_{i=0}^{N-1} (\text{gain} \cdot \text{lac})^i$ | For $\text{gain}=0.5, \text{lac}=2$, sum $= N$ (grows linearly with octaves!) |
| Division over $0 \in I(g)$, unbounded `exp`/`tan`, `raw wgsl`, unproven runtime loop | **`Unbounded`** | Never guessed. `trace` rejects `Field[S, Unbounded]` unless wrapped in `bound_step(f, max_step)` (conservative fixed-step + bisection) |

---

## 3. Explicit Staging & Metaprogramming Model

### The Three-Stage Boundary

1. **Stage 0 (`comptime` — topology & table emission):** Runs on CPU/WASM at scene build. `comptime for` emits **table rows** via `emit Kind { ... }`, never unrolled shader ASTs.
2. **Stage 1 (`kind` — sole unit of code identity):** A `kind` compiles to **one** canonical shader family. `kind` parameters (`kind Foo[comptime mode: Mode]`) are strictly limited to **discrete structural types** (`bool`, `enum`, lobe bitsets, small bounded `u32` with domain size at most 8). Continuous types (`f32`, `Meters`, `Rad`, `vec3`) are **compile errors** in `comptime` kind parameters—they must live in `row { ... }`.
3. **Stage 2 (`runtime` — GPU/CPU execution):** `for` loops inside `kind` functions stay real WGSL loops. Instance variation always reads `self.<field>` from the kind's SoA/AoS row buffer.

### Who Owns Dispatch Shape?

**Compiler cost model by default, schedule declarations as overrideable hints:**

* The compiler automatically performs **(1) dependency-wave topological batching**, **(2) bucketing by `(wave, law_kind)**`, and **(3) kind-level shader deduplication**. Authors cannot accidentally trigger 5,539 modules or 4,884 sequential pose dispatches.
* A `schedule` block declares *algorithmic topology* (e.g., vis-buffer vs forward, tile size, whether a `scan` is baked or per-frame) and optional overrides (`@fuse_if(regs <= 48)`, `@inline_max(64)`). If an author override violates a hardware budget, the compiler warns and CI gates catch the frame-time/module regression.

### Example: "12 Legs, Each With Its Own Length" (1 Kind, 12 Rows)

```wgsl
kind CrabLeg {
  row {
    parent:    RowId[CrabBody],
    socket:    Xform[Leg -> Body],
    coxa_len:  Meters,
    femur_len: Meters,
    tibia_len: Meters,
    side:      f32,
    phase:     Rad,
  }

  query visible(self, u: Unit, v: Rad) -> { p: Pt[Leg], n: Norm[Leg] } {
    // Runs the SAME shader for all 12 legs (and all 240 legs across 20 crabs)
    let spine_p = leg_ik_point(self.coxa_len, self.femur_len, self.tibia_len, u);
    let r = mix(0.11m, 0.035m, u);
    // ... parametric tube evaluation ...
  }
}

// Stage 0: loop appends 12 rows to the CrabLeg storage table -> 0 new shaders
comptime fn spawn_crab_legs(body: RowId[CrabBody]) {
  comptime const FEMUR: array<Meters, 6> = [0.62m, 0.78m, 0.88m, 0.84m, 0.70m, 0.54m];
  comptime for pair in 0u..6u {
    comptime for side in [-1.0, 1.0] {
      emit CrabLeg {
        parent:    body,
        socket:    leg_socket_xform(pair, side),
        coxa_len:  0.22m,
        femur_len: FEMUR[pair],
        tibia_len: FEMUR[pair] * 1.18,
        side:      side,
        phase:     f32(pair) * 1.047rad,
      };
    }
  }
}

```

### Error When an Author Accidentally Makes 12 Kinds Instead

```text
error[E401]: continuous value cannot parameterize kind identity
  --> crab_legs.loft:1:15
   |
 1 | kind CrabLeg[comptime femur_len: Meters] {
   |               ^^^^^^^^^^^^^^^^^^^^^^^^^ `Meters` is continuous; kind parameters must be discrete
   |
   = note: instantiating this in `spawn_crab_legs` would mint 12 distinct shader modules.
   = fix: move `femur_len: Meters` into the `row { ... }` block so all legs share one compiled kind.

```

*(If an author instead tries a discrete `u32` leg index `0..12` as a `comptime` parameter, `E402: kind variant budget exceeded (12 > max 4)` triggers with the same one-sentence fix.)*

---

## 4. Tier 0 / Tier 1 Contract (Calibrated to Your +2.0 ms / 63-Op Data)

Your measurement (+2.0 ms for 63 ops, +5.5 ms for larger graphs) proves two things: **(a)** a global tape VM cannot run the whole scene in production, and **(b)** running *only the single actively edited kind* in a workgroup-uniform tape VM is completely viable at 60–120 Hz while Metal compiles Tier 1 asynchronously.

### 1. Opcode Set (~56 Fixed 32-bit 3-Address SSA Ops)

* **Included:** scalar/vec3/vec4 math (`fma`, `add`, `mul`, `min`, `max`, `clamp`, `mix`, `smoothstep`, `smootherstep`, `dot`, `cross`, `len`, `norm`), transcendentals (`sin`, `cos`, `exp`, `log`, `pow`, `sqrt`), SDF primitives + `smin_poly`/`smin_exp`, transforms (`xform_pt`, `xform_dir`, `mirror`, `repeat_cell`), and **fused noise intrinsics** (`grad_noise3`, `smooth_voronoi3` as single opcodes—never 40 scalar ops).
* **Loops:** **Yes** — structured `LOOP_BEGIN(trip_reg, end_pc) / LOOP_END` with static trip guard (at most 64 iterations).
* **Table & Texture Reads:** **Yes** — `LOAD_ROW(col_offset)`, `SAMPLE_LUT_C2(table_slot, t)`, `SAMPLE_TEX2D(tex_slot, uv)` via fixed bind-group descriptor arrays.
* **Derivatives (`diff`):** **Lowered before tape emission.** Forward-mode AD runs on the SSA graph in Rust (< 0.5 ms), emitting ordinary arithmetic opcodes into the tape. No runtime AD opcode exists.
* **Excluded:** `raw wgsl`, workgroup shared memory, atomics, subgroups. Kinds using those skip Tier 0 and keep their previous Tier 1 pipeline live until async compilation finishes.

### 2. SIMD Divergence Rule

A shared tape interpreter running heterogeneous kinds across lanes in one 32-thread Apple SIMD group **does diverge catastrophically** (every opcode switch diverges and lanes stall until the longest tape exits).

**Mandatory invariant:** Tier 0 is **never** dispatched heterogeneously. Every Tier 0 dispatch is **kind-bucketed** (one indirect dispatch per dirty Tier 0 kind; `tape_offset` is workgroup-uniform). Every thread in a 32-lane SIMD group executes the **exact same PC and opcode**—eliminating control-flow divergence entirely. The only remaining overhead is register-array indexing and opcode fetch, and because **clean kinds stay on Tier 1**, editing 1 crab kind covering ~12% of the screen pays **~0.25–0.45 ms** instead of +2.0 ms full-screen.

### 3. Promotion Policy (Tier 0 $\to$ Tier 1)

* **On edit:** Dirty kind immediately uploads its 2–8 KB tape buffer (**0 ms Metal compile, under 2 ms hot-reload**).
* **Promote to Tier 1 when:** The kind's canonical IR hash is quiet for **400 ms** (or **0 ms** on scene load / CI / release) AND (`ops > 20` OR measured GPU pass time `> 0.35 ms`).
* **Swap:** Compile native WGSL via `create*PipelineAsync` on a worker, cache in an LRU keyed by canonical IR hash (so undo/redo swaps in 0 ms), and atomically flip the kind's dispatch handle between frames.

### 4. Parity & Swap Tolerance Contract

Tier 0 and Tier 1 **will not be bit-identical** because Metal's LLVM backend contracts `a * b + c` into hardware FMA and uses fast transcendental approximations.

* **Contract:**
1. Both backends emit explicit `fma()` calls and share the exact polynomial definitions for `grad_noise3`, `smooth_voronoi3`, and `smin_*`.
2. **Field / scalar eval:** $\vert{}\text{Tier0} - \text{Tier1}\vert{} \le 10^{-5}$ abs (or 8 ULP).
3. **Motion / solver laws:** **Do not run in Tier 0** unless deterministic IEEE mode is active; motion kernels are tiny (1–2 modules, < 100 ms compile) and stay bit-matched to the CPU twin.
4. **Shaded swap continuity:** sRGB pixel diff $\le 1/255$ on at least $99.9\%$ of covered pixels ($\Delta E_{2000} \le 0.5$), max $2/255$ on grazing silhouette edges. Verified per-opcode in CI so mid-session promotion never visibly pops.



---

## 5. Scans, Race-Free Solvers, Motion & Open 6-Query Rendering

### A. Scans (`parallel_transport`)

`scan` explicitly declares sequential integration along a parameter:

* `@schedule(bake_lut)` — runs once at load (0 ms/frame) when the spine depends only on constants/row-static shapes.
* `@schedule(per_frame_compute)` — placed in a motion dependency wave before dependent geometry when the spine animates (e.g., flexing neck).
* **Differentiability:** Continuous sampling uses `sample_c2(table, t)` (cubic Hermite, provably $C^2$ in $t$). Differentiating w.r.t. upstream curve parameters automatically generates the forward-tangent scan pass.

### B. Race-Free GPU Solvers & Checkpointed State

* Parallel neighbor mutation is forbidden in raw kernels. `solver` requires an explicit conflict-free strategy:
* **`gauss_seidel(colored: 2)`** for chains/trees: phase 0 solves even edges $(2k, 2k+1)$, phase 1 solves odd edges $(2k+1, 2k+2)$—compiler proves disjoint writes. When strand length $\le 64$, the compiler maps 1 workgroup per strand and runs all substeps and colored iterations inside **`var<workgroup>` shared memory in 1 dispatch**.
* **`jacobi(omega: 0.5)`** for arbitrary meshes: gather/atomic delta buffer + normalized apply pass.


* **State ownership & scrubbing:** `state` buffers live in Layer 0's **State Store** with `@checkpoint(every: 60)` ring-buffer snapshots. Scrubbing to frame $F$ restores the latest checkpoint $\le F$ and steps forward deterministically ($\le 59$ substeps, < 2 ms) so scrubbing matches live execution to the bit.

### C. First-Class Motion & Honest Verdicts

* Joints/targets take explicit `Attach[Body]` vs `Target[World]` types so rigid body-locking cannot masquerade as world stabilization.
* Constraints return a **three-valued `Verdict**`:
* `Verdict::Held(margin: Meters)` — satisfied with positive margin.
* `Verdict::Missed(err: Meters)` — failed with exact magnitude (e.g. `Missed(0.003m)`).
* `Verdict::Unknown(reason)` — NaN, solver hit iteration cap, missing contact, or missing frame. CI gates pass **only** on `Held`.



### D. Mapping Loft's 6 Queries Without Freezing the Renderer

No hardcoded `GBuffer` or `PbrSurface`. Each `kind` implements any subset of `trace`, `visible`, `material`, `light`, `sky`, `medium`:

* `material` returns an **open algebraic `Bsdf**` (`ggx(...) + clearcoat(...) + thin_film(...) + sheen(...) + sss(...) + emission(...)`). Adding a `thin_film` lobe to one kind requires zero changes to any visibility buffer or other kind.
* `visible` writes a **64-bit Visibility Record** `(kind_id: u16, row_id: u24, prim_or_uv: u24)`—geometry transport is completely decoupled from shading.
* A `schedule` freely mixes **deferred visibility-buffer** (opaque shell/claws/legs), **forward** (translucent eyes/water reading `medium` + `sky`), and **path-traced `trace` rays** (wet reflections/GI) in the same frame.

---

## 6. The Hermit Crab — Rewritten & Verified

```wgsl
space World, Body, Shell, Claw, Leg;
unit Meters; unit Rad; unit Sec; unit Kg;

// --- 1. Parallel-Transport Scan (Baked Once at Load, C2-Sampleable) ---
fn shell_spine(t: Rad) -> Pt[Shell] {
  let e = exp(0.18 * f32(t));
  return Pt[Shell](e * 0.95 * cos(t), e * 0.95 * sin(t), -e * 0.72) * 1.0m;
}

scan shell_frames(t: Rad in 0.0rad..28.0rad, steps: 256u) @schedule(bake_lut) -> Table[256, Frame[Shell]] {
  init: frame_from_tangent(shell_spine(0.0rad), normalize(diff(shell_spine, t)(0.0rad)), Dir[Shell](0.0, 0.0, 1.0));
  step(prev, t_i): transport_frame(prev, shell_spine(t_i), normalize(diff(shell_spine, t)(t_i)));
}

// --- 2. Gastropod Shell Kind (C2 Macro Curvature + C1 Smooth-Voronoi Barnacles) ---
kind HermitShell {
  row {
    pose: Xform[Shell -> World],
    aggro: Unit,
  }

  // Strictly C2: uses smooth cosine power (no C0 max/abs, no Voronoi)
  fn macro_surface(t: Rad, a: Rad) -> Pt[Shell] {
    let f     = sample_c2(shell_frames, t);
    let scale = exp(0.18 * f32(t)) * 0.62m;
    let rib   = 0.08 * pow(0.5 + 0.5 * cos(6.0 * t), 4.0) * (0.4 + 0.6 * sin(5.0 * a));
    return f.origin + f.normal * (scale * (1.15 + rib) * cos(a))
                    + f.binormal * (scale * (0.82 + rib) * sin(a));
  }

  // C1 micro-displacement kept separate so macro curvature never differentiates through Voronoi
  fn barnacle_height(t: Rad, a: Rad) -> Meters {
    return 0.035m * smooth_voronoi_cone(vec3(f32(t) * 2.1, f32(a) * 3.0, 0.0), k: 0.12);
  }

  query visible(self, t: Rad, a: Rad) -> { p: Pt[World], n: Norm[World], crevice: Unit } {
    let p0  = macro_surface(t, a);
    let du  = diff(macro_surface, t)(t, a);              // C2 -> C1
    let dv  = diff(macro_surface, a)(t, a);              // C2 -> C1
    let n0  = normalize(cross(du, dv)) as Norm[Shell];
    let d2u = diff(du, t);                               // C1 -> C0 (legal! would error on Voronoi)
    let k   = dot(d2u, n0) / max(1e-5, dot(du, du));

    // Apply C1 barnacles along macro normal; first-order diff gives valid C0 perturbed normal
    let h   = barnacle_height(t, a);
    let dh  = vec2(diff(barnacle_height, t)(t, a), diff(barnacle_height, a)(t, a));
    let n_s = normalize(n0 - normalize(du) * dh.x - normalize(dv) * dh.y) as Norm[Shell];
    return {
      p: self.pose * (p0 + n0 * h),
      n: self.pose * n_s,
      crevice: smootherstep(0.4, -0.6, k),
    };
  }

  query material(self, hit) -> Bsdf {
    let grime = saturate(hit.crevice * 1.2 + 0.25 * grad_noise3(hit.local_p * 4.0));
    let base  = mix(rgb(0.62, 0.48, 0.36), rgb(0.09, 0.21, 0.14), grime);
    return ggx(albedo: base, roughness: mix(0.28, 0.78, grime))
         + clearcoat(weight: 1.0 - grime * 0.85, roughness: 0.08)
         + thin_film(weight: 0.25 * (1.0 - grime), ior: 1.42, thickness_nm: 380.0); // nacre sheen!
  }
}

// --- 3. Crusher Claw (Compiler-Derived L = 1.41, No FBM Inside Distance Field) ---
kind CrusherClaw {
  row {
    pose:     Xform[Claw -> World],
    jaw_open: Rad,
    aggro:    Unit,
    side:     f32,                                       // +1 crusher, -1 feeder via row scale
    scale:    f32,
  }

  // Return type omits L: compiler proves L <= 1.41 from round_box(1) + teeth(0.41) + polar clamp(1)
  query trace(self, p_world: Pt[World]) -> Field[Claw] {
    let p      = inv(self.pose) * p_world;
    let palm   = ssub_poly(sd_round_box(p, vec3(0.65, 0.35, 0.95)m, 0.28m),
                           sd_box(p - Vec[Claw](0.1, -0.35, 0.2)m, vec3(0.7, 0.3, 0.9)m), 0.06m);
    map ring   = repeat_polar(p.xz, 7u);                 // auto cell-wall clamped
    let studs  = sd_sphere(ring - Pt['ring](0.75m, 0.0m, 0.0m), 0.11m);
    map jaw    = (p - Vec[Claw](0.0, -0.22, 0.85)m) |> rot_x(-self.jaw_open);
    let teeth  = 0.025m * sin(14.0 * f32(jaw.z)) * smootherstep(0.1m, 0.9m, jaw.z);
    let dactyl = sd_capsule(jaw, Pt['jaw](0.0, 0.0, 0.0)m, Pt['jaw](0.0, -0.15, 1.15)m, 0.24m) - teeth;
    return smin_poly(smin_poly(palm, dactyl, 0.08m), studs, 0.04m);
  }

  query material(self, hit, time: Sec) -> Bsdf {
    // High-frequency chitin pitting lives at shade time (0 impact on raymarch L!)
    let micro = grad_noise3(hit.local_p * 6.5);
    let veins = smootherstep(0.62, 0.78, smooth_voronoi3(hit.local_p * 5.2)) * self.aggro;
    return ggx(albedo: rgb(0.68, 0.14, 0.07), roughness: mix(0.16, 0.42, micro))
         + clearcoat(weight: 0.9, roughness: 0.06)
         + sss(color: rgb(0.9, 0.25, 0.08), radius: 0.35)
         + emission(rgb(1.0, 0.32, 0.04) * veins * (3.5 + 1.5 * sin(f32(time) * 8.0)));
  }
}

// --- 4. Expressive Face & Eyestalks (Scoped Mirror Space Fixes World Gaze) ---
kind CrabFace {
  row {
    pose:   Xform[Body -> World],
    aggro:  Unit,
    target: Target[World],                               // explicit world-space gaze target
  }

  query trace(self, p_world: Pt[World], time: Sec) -> { d: Field[Body], is_pupil: Unit } {
    let p     = inv(self.pose) * p_world;
    map sym   = mirror_x(p) |> translate(-Vec[Body](0.42, 0.55, 1.18)m);

    map brow  = (sym - Vec['sym](0.0, 0.28, 0.05)m)
              |> rot_z(-0.35rad * self.aggro) |> rot_x(0.28rad * self.aggro);
    let plate = sd_wedge(brow, vec3(0.32, 0.10, 0.38)m);

    let sway  = 0.08m * sin(f32(time) * 2.1);
    let eye_c = Pt['sym](sway, 0.24m, 0.16m);
    let stalk = sd_capsule(sym, Pt['sym](0.0, -0.2, 0.0)m, eye_c, 0.09m);
    let globe = sd_sphere(sym - eye_c, 0.135m);

    // Exact world-space eye position for BOTH left and right eyes via sym.to_parent (uses sym.cell!)
    let eye_world = self.pose * (sym.to_parent * eye_c);
    let look_w    = normalize(self.target.pos - eye_world);
    let look_sym  = sym.from_parent.dir(inv(self.pose) * look_w);
    let pupil     = smootherstep(0.88, 0.94, dot(normalize(sym - eye_c), look_sym));

    let d = smin_poly(smin_poly(stalk, globe, 0.05m), plate, 0.03m);
    return { d, is_pupil: pupil * select(0.0, 1.0, globe < stalk) };
  }
}

// --- 5. Race-Free Colored XPBD Antennae (Pinned to Head, Checkpointed State) ---
solver AntennaSolver over AntennaStrands {
  @checkpoint(every: 60)
  state { pos: array<Pt[World], 16>, prev: array<Pt[World], 16>, inv_m: array<1/Kg, 16> }

  row {
    parent:    RowId[CrabBody],
    head_pin:  Attach[Body],                             // explicit rigid attachment to head socket
    rest_len:  Meters,
    stiffness: f32,
  }

  config { strategy: gauss_seidel(colored: 2), substeps: 4u, iterations: 6u, shared_mem: true }

  step(dt: Sec, time: Sec, bodies: Table[CrabBody]) {
    let root_w = bodies[self.parent].pose * self.head_pin.offset;
    // Predict + pin node 0 before constraint sweeps
    pos[0] = root_w; prev[0] = root_w; inv_m[0] = 0.0 / 1.0kg;
    for i in 1u..16u {
      let current = Vec[World](sin(f32(time) + f32(i) * 0.4), 0.2, cos(f32(time) * 0.7)) * 1.4m/(1.0s*1.0s);
      let v = (pos[i] - prev[i]) / dt;
      prev[i] = pos[i];
      pos[i]  = pos[i] + (v * 0.96 + current * dt) * dt;
    }
    // Compiler schedules phase 0 (even edges) and phase 1 (odd edges) with workgroupBarrier()
    constrain edge(i, i + 1u) in 0u..15u {
      solve_dist(&pos[i], &pos[i + 1u], inv_m[i], inv_m[i + 1u], self.rest_len, compliance: 0.002);
    }
  }
}

// --- 6. First-Class Motion Law & Honest Constraint Verdicts ---
law CrabStepLaw(dt: Sec, bodies: Table[CrabBody], legs: Table[CrabLeg]) writes CrabLeg {
  // Capture-point foot stepping: steps from current foot pos when weight shifts past support polygon
  let cp = bodies[leg.parent].com_world + bodies[leg.parent].vel_world * 0.28s;
  if distance(leg.foot_world, leg.nominal_anchor(cp)) > leg.stride_limit {
    leg.step_target = Target[World](project_seabed(cp + leg.foot_offset));
  }
}

constraint EyeGazeAndPosture(b: CrabBody, face: CrabFace) -> Verdict {
  if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
  let head_w = b.pose * b.head_socket;
  let margin = head_w.y - (b.hip_world.y + 0.15m);
  if margin < 0.0m { return Verdict::Missed(-margin); }  // e.g. "Missed by 3.2 mm"
  return Verdict::Held(margin);
}

// --- 7. Open 6-Query Hybrid Schedule (No Hardcoded G-Buffer or PbrSurface) ---
schedule SeaBossHybrid {
  // Compiler buckets laws automatically by (wave, law_kind)
  pass motion:   run_laws(CrabBodyLaw -> CrabStepLaw);
  pass physics:  run_solver(AntennaSolver);
  pass cull:     build_indirect(HermitShell, CrabLeg, AntennaRibbon, CrusherClaw, CrabFace);
  // 64-bit Visibility Buffer: adding thin_film or new kinds never changes buffer layout
  pass vis:      render_bundle(Vis64) {
                   draw_indirect_mesh(HermitShell, CrabLeg, AntennaRibbon);
                   draw_indirect_trace_proxy(CrusherClaw, CrabFace);
                 };
  pass classify: tile_bin_kinds(Vis64, tile: 8x8);
  pass shade:    dispatch_material_tiles(Vis64, light: SunCaustics, sky: OceanSky, medium: DeepWater);
  pass present:  tonemap_aces(ColorHdr) -> Swapchain;
}

```

---

## 7. Lowering & Draw-Path Portability (1 Crab vs 20 Crabs)

### How WebGPU Core Achieves $O(K_{\text{kinds}})$ Without `multi_draw_indirect`

Browser WebGPU lacks `multi_draw_indirect` and indirect draw counts, **but it supports `drawIndirect` inside a `RenderBundle**`. Because staging guarantees every instance of a kind shares one compiled topology/proxy and differs only by its row index:

1. One `cull` compute dispatch tests all rows (using compiler-derived interval AABBs), compacts visible `row_id`s per kind into a contiguous `visible_rows` buffer, and writes **one 16-byte `DrawIndirectArgs` per kind** (`instance_count = visible_count`).
2. A **pre-recorded `RenderBundle**` (rebuilt only when the set of kinds changes, never per frame) executes **1 `setPipeline` + 1 `drawIndirect` per kind**—both for instanced meshes (`HermitShell`, `CrabLeg`, `AntennaRibbon`) and for SDF bounding-box proxies (`CrusherClaw`, `CrabFace`).
3. If `visible_count == 0`, the GPU skips the draw at zero vertex cost. Native `wgpu` only needs `multi_draw_indirect_count` if you later pack dozens of *distinct static prop topologies* under a single shared pipeline.

### Lowering Breakdown (Apple M4, 1280×800)

| Metric | 1 Crab (Native `wgpu`) | 20 Crabs (Native `wgpu`) | 1 Crab (Browser WebGPU) | 20 Crabs (Browser WebGPU) |
| --- | --- | --- | --- | --- |
| **WGSL Shader Modules** | **11** | **11** | **11** | **11** |
| **Compiled Pipelines** | **12** | **12** | **12** | **12** |
| **Cold / Warm Pipeline Load** | ~1.1 s / ~0.15 s | ~1.1 s / ~0.15 s | ~1.4 s / ~0.20 s | ~1.4 s / ~0.20 s |
| **Compute Dispatches / Frame** | **7** | **7** | **7** | **7** |
| **Render Draw Commands / Frame** | **6** (5 in bundle + 1 present) | **6** (5 in bundle + 1 present) | **6** (5 in bundle + 1 present) | **6** (5 in bundle + 1 present) |
| **Total GPU Commands** | **13** (~0.20 ms cmd cost) | **13** (~0.20 ms cmd cost) | **13** (~0.20 ms cmd cost) | **13** (~0.20 ms cmd cost) |

#### Per-Pass GPU Time Breakdown at 1280×800 (1.024M px, M4)

| Pass | 1 Crab (ms) | 20 Crabs (ms) | What Scales From 1 $\to$ 20 |
| --- | --- | --- | --- |
| Motion waves (2 dispatches: body $\to$ 240 legs IK) | 0.04 | 0.08 | 20 bodies, 240 legs (0 extra dispatches) |
| Antennae XPBD (1 dispatch, shared-mem colored) | 0.02 | 0.07 | 4 $\to$ 80 workgroups (1 workgroup/strand) |
| Cull + indirect args (1 dispatch) | 0.02 | 0.03 | 19 $\to$ 380 rows tested |
| Vis-buffer mesh raster (shell + 12×N legs + ribbons) | 0.18 | 0.55 | ~11k $\to$ ~140k visible tris (after frustum cull) |
| Vis-buffer SDF proxy trace (claws + faces, early-Z) | 0.65 | 1.65 | ~140k $\to$ ~350k unoccluded px (derived $L=1.41$) |
| Tile classify + Deferred tile PBR/caustics (3 dispatches) | 0.63 | 1.05 | ~256k $\to$ ~665k fg pixels (zero SIMD divergence) |
| Tonemap + present (1 draw) | 0.12 | 0.12 | Constant (full-screen) |
| Driver/encoder floor + 13 commands | 0.45 | 0.45 | Constant (+0.6 ms Dawn floor in browser) |
| **Predicted Total GPU Frame Time (p50 / p95)** | **2.1 / 2.4 ms** | **4.0 / 4.6 ms** | **Browser WebGPU: 2.7 ms (1 crab) / 4.7 ms (20 crabs)** |

---

## 8. Three Type-Error Diagnostics for Model Authors

### 1. Space Error (Catching the Original Gaze Bug)

```text
error[E104]: space mismatch in transform application
  --> hermit_crab.loft:94:22
   |
83 |   map sym   = mirror_x(p) |> translate(-Vec[Body](0.42, 0.55, 1.18)m);
   |       --- space `'sym` introduced here
...
94 |   let eye_world = self.pose * eye_c;
   |                   ^^^^^^^^^   ^^^^^ expected `Pt[Body]`, found `Pt['sym]`
   |
   = fix: lift `eye_c` back to `Body` space via `self.pose * (sym.to_parent * eye_c)` so the offset and mirror sign are inverted.

```

### 2. Unproven Lipschitz Bound

```text
error[E211]: sphere tracer requires a proven Lipschitz bound, got `Field[Claw, Unbounded]`
  --> hermit_crab.loft:66:5
   |
64 |   let flare = 0.04m / max(0.0, length(p.xz) - 0.5m);
   |               ------------------------------------- interval of denominator `[0.0, 1.4]` contains 0 -> L = Unbounded
66 |   return smin_poly(palm, dactyl, 0.08m) - flare;
   |   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ returned from `query trace`
   |
   = fix: clamp the denominator away from zero (`max(0.05m, ...)`) or wrap the field in `bound_step(..., max_step: 0.04m)` for conservative fixed-step marching.

```

### 3. Non-Smooth Second Derivative (Catching the Voronoi Curvature Bug)

```text
error[E307]: cannot differentiate a `C0` expression
  --> hermit_crab.loft:38:15
   |
29 |   let barn = 0.035m * voronoi_cone(vec3(t * 2.1, a * 3.0, 0.0));
   |                       ------------ `voronoi_cone` is `C0` (gradient discontinuous at cell borders)
...
38 |   let d2u  = diff(du, t);
   |              ^^^^^^^^^^^ `du` has smoothness `C0`, but `diff` requires at least `C1`
   |
   = fix: compute curvature on the `C2` macro shell before adding barnacles, or switch to `smooth_voronoi_cone` and perturb normals with first-order `diff` only.

```

---

## 9. The Single Change That Matters Most After Seeing Your Numbers

My earlier proposal leaned on **post-hoc compiler deduplication** (`egglog`/hash-consing) and a **blanket Tier 0 Tape VM** to rescue hot-reload and sharing. Your M4 numbers kill that approach: once host tracing unrolls Titan into 707k nodes or fractures five crabs into 305–5,539 modules, deduplication is fighting the tide, and running the whole frame through a tape VM costs +2.0–5.5 ms.

The single highest-leverage architectural change is **enforcing two-level staging (`kind` = discrete code identity, `row` = continuous instance data) as a hard type-system law at the language frontier, paired with dirty-only, workgroup-uniform Tier 0 tape execution**. That one rule makes module count strictly $O(K_{\text{kinds}})$ by construction—collapsing 32.9 s cold compiles to ~1 s, making `drawIndirect` + `RenderBundle` portable across browser WebGPU and native `wgpu` with zero per-instance draw scaling, and keeping Tier 0 editing overhead under 0.5 ms while clean kinds run natively on Tier 1.


================================================================
# RENDER ARCHITECTURE
# file: docs/proposals/2026-11-19_render_architecture.md
# what it is: our consolidated render design (evidence, not instructions): kinds, six queries, schedules
================================================================

# Render and light: one architecture (19 Nov 2026)

Status: proposal, nothing built. It consolidates three independent designs for the render and light sub meta
engines into one plan. Read `docs/PHILOSOPHY.md` (§0, §0.5), `docs/design/render.md` and `CORE.md` first.

**What I took the request to be.** Make one architecture from three designs. Where they differ, pick the stronger
idea and say why. Graft in the best of the rest and mark what is inherited and what is ours. Keep the consumer's code
first and the performance theory to the number, with speed meaning GPU frame time first, then load time and module
count. Every built-in is a rewritable example engine and the core stays tiny. Apply the lead's three decisions,
decide what happens to wire 4's draw declarations in light of the draws lane's measured baseline, and end with
phases, the first buildable by one implementer in a worktree.

**What I read.** The three pages. The designs: Fable's (`agent_notes/2026-11-19_design/render_fable_step{1,2}.md`),
Astra's high-effort pass (`high_effort_run/render_astra_step1.md`, `render_astra_step2.md`) and Astra's philosophy
pass (`philosophy_pass/render_step{1,2}.md`). Fable's high-effort critique, the third-party and cut audits, the lean
Rust plan §1.5, §1.6 and §2.2–2.3. The draws lane, merged to main as `4e8ee7b`: its note
(`agent_notes/2026-11-19_lean_rust_draws.md`), `loft-ir/src/bundle.rs`, `loft-core/src/draw/mod.rs`, the 13 WGSL
templates, `src/draw/record.ts`, and its strict full parity run (`out/parity/rust/rust.json`, head `963c667`). I
parsed that run's 229 built bundles for modules, WGSL lines, kernels and draws. I ran nothing on the GPU.

**Marks.** `[repo]` measured in this repository. `[fit]` a least-squares fit I made today over that run. `[derived]`
arithmetic on the numbers beside it. `[conjecture]` a constant with a band, scored by the phase named. Token counts
are `o200k_base` over the exact block shown. The sources are **F** (Fable), **A** (Astra, high effort) and **P**
(Astra, philosophy pass).

## 0. The answer on one page

A Loft drawable answers "how is this thing drawn". F split that into three questions with three owners. The split is
the spine of this design.

1. **What is it?** A **kind**: a WGSL struct and the readings written for it. A shape gives `hit`, `bound`, `dist`,
   `place` or `gap`. A surface response gives `eval`, `sample`, `emit` or `albedo`. A medium gives `sigma` or `tau`.
   A light gives `light`. Instances are rows. The consumer writes kinds.
2. **Who is seen, and what light arrives?** Six queries over a **world**: `trace`, `visible`, `material`, `light`,
   `sky`, `medium`. The world is the scene's things unless an engine binds another to one class of ray (P). An
   estimator is any function that calls the queries.
3. **When, how often, by what method?** A **schedule**: the pass, the rate, the method and the compile tier. The
   compiler chooses from a cost model, and the consumer overrides it by passing a function. Laws do not change when
   the schedule does.

An **engine** is a pass list over these. Loft ships four example engines, written only with public pieces:
`realtime` (the default), `print` (toon, woodblock, the corpus looks), `reference` (the path tracer that judges
light) and `inspect` (one screen pass, the demoscene shape).

| # | Question | Decision | From |
|---|---|---|---|
| 1 | Unit of extension | Kinds with readings. Holes and constructor arguments are row data, never code | F |
| 2 | Primary visibility | A visibility record per coverage sample (4×, `rgba16uint` and depth); the shade pass shades each distinct thing in a pixel once | F, made parity-safe here |
| 3 | Render–light seam | Six queries closed over a world value; `visible` is an any-hit loop that reads only `t` | F, P, A |
| 4 | Material contract | `eval`, `sample`, `emit`, `albedo`, with an explicit delta tag; adapters derive what is missing | F, P, A |
| 5 | Media | One in-scatter pass through the sum of all media; RGB optical depth; (L, T) composition | F, P, A |
| 6 | History | Viewer: one light sample a pixel plus a screen history keyed by the record. Stills, clips, training: nothing carried between frames | lead, F |
| 7 | GPU code count | One module per pass, kinds dispatched by a generated switch; kernels at one clock and level fused into one dispatch | F, this note |
| 8 | Specialisation | Never for a clip; in the viewer only for a site hot for over a minute | F, recomputed |
| 9 | Substitution | An adapter stands in for a reading only for the observations it reproduces exactly; otherwise it is a named approximation scored against the reference | P |
| 10 | Judging | Shape, silhouette, depth: picture parity against R0 on a light-free look. Light: against a converged `reference` | lead |
| 11 | Wire 4 | **Adapted** (§4): covers, cameras, raster state, packs, the recorder kept; paint returning rgba, `Compose`, `Medium`, `Target::Shadow`, a module per draw and a module per kernel replaced | this note |
| 12 | Core | Five nouns: function, table, kind, pass, rate. No rendering words in Rust | F, P |

Why act now: on the Rust path the five-crab lineup has **5,538 shader modules, 2,259,509 WGSL lines and 5,533 GPU
commands a frame**; it loads in **23.1 s** and draws at **97.2 ms p50** at 960 × 540. The TS engine draws it at
4.26 ms from 305 modules `[repo]`. The straight port of the inlined graph made module count and frame time worse,
and §5.1 shows why: commands, not pixels, dominate the large scenes.

## 1. The consumer's code first

Written before the architecture and read cold. `things`, `camera`, `ring` and `SUN` in later cases are those of
earlier ones.

### Case 1. A scene with five methods in one frame (525 tokens, F's case 1 unchanged)

```ts
import { scene, thing, at, look, wgsl } from "loft";
import { plane, sphere, box, field, surface, matte, metal, glass, glow, fog, sky } from "loft/kinds";

const k = wgsl`
fn knot(p: vec3f) -> f32 {                        // a distance: three blended balls
  let a = length(p - vec3f(0, 0.25, 0)) - 0.35;
  let b = length(p - vec3f(0.3, -0.1, 0)) - 0.3;
  let c = length(p - vec3f(-0.3, -0.1, 0)) - 0.3;
  return smin(a, smin(b, c, 0.15), 0.15);
}
fn vase(uv: vec2f) -> vec3f {                     // a surface: a profile turned about y
  let r = 0.25 + 0.1 * sin(6 * uv.y);
  return vec3f(r * cos(TAU * uv.x), uv.y, r * sin(TAU * uv.x));
}`;

export default scene({
  things: [
    thing(plane(), matte([0.5, 0.5, 0.5])),
    thing(sphere(0.5), metal([1, 0.8, 0.5], 0.2), at([-1.2, 0.5, 0])),
    thing(field(k.knot, sphere(0.7)), matte([0.8, 0.2, 0.1]), at([0, 0.45, 0])),
    thing(surface(k.vase), glass(1.5), at([1.2, 0, 0])),
    thing(sphere(0.12), glow([40, 22, 8]), at([0.9, 1.4, 0.3])),   // anything that glows is a light
    thing(box([4, 2, 4]), fog(0.08), at([0, 1, 0])),               // a medium fills its shape
  ],
  sky: sky([-0.4, 0.8, 0.45]),
  camera: look([3, 2, 4], [0, 0.5, 0]),
});
```

Planes and spheres are intersected exactly, the knot is sphere-traced inside its bound, the vase is rasterised as a
grid, the fog is integrated in closed form. Nothing names a method. `--engine reference` draws the same file with the
path tracer.

### Case 2. A new primitive (263 tokens)

```ts
const k = wgsl`
struct Washer { r0: f32, r1: f32 }                  // a flat ring in z = 0: inner and outer radius
fn bound(w: Washer) -> Box { return Box(vec3f(-w.r1, -w.r1, -1e-3), vec3f(w.r1, w.r1, 1e-3)); }
fn hit(w: Washer, r: Ray) -> Hit {
  if (r.d.z == 0) { return miss; }                   // a ray in the plane meets no single point
  let t = -r.o.z / r.d.z;
  let p = r.o.xy + t * r.d.xy;
  let q = dot(p, p);
  if (t < r.t0 || t >= r.t1 || q < w.r0 * w.r0 || q > w.r1 * w.r1) { return miss; }
  return Hit(t, vec3f(0, 0, 1), p);                  // p is the chart: materials and caches index by it
}`;

const ring = thing(k.Washer(0.5, 1), brass, at([0, 0, 1]));
```

From this one definition the washer is found by the camera, casts and receives shadows, is met by bounces and
reflections, answers a pick, appears in `reference` and has a CPU twin. None of that is written here. Widening the
hole changes a row, not a module (Phase 1, check 2). Today's `examples/bead.ts` is 979 tokens (F), and A found that
its public hit returns a solid hit for a ray straight down the bore.

### Case 3. A new BRDF (231 tokens, F's case 3)

```ts
const k = wgsl`
struct Shell { base: vec3f, turn: vec3f, rough: f32 }
fn eval(m: Shell, wo: vec3f, wi: vec3f) -> vec3f {             // f · cos, in the surface frame (z is the normal)
  let h = normalize(wo + wi);
  let hue = mix(m.base, m.turn, pow(1 - dot(wo, h), 2));       // the colour turns toward grazing
  return hue * ggx(m.rough, wo, wi);
}
fn stripes(x: Point) -> Shell {                                // a material may vary over the surface
  return Shell(vec3f(0.1, 0.4, 0.2), vec3f(0.5, 0.1, 0.6), mix(0.15, 0.5, step(0.5, fract(8 * x.uv.x))));
}`;

thing(sphere(0.5), k.stripes, at([0, 0.5, 0]))
```

`eval` is enough. Adapters derive `sample` (cosine-weighted) and `albedo`; writing `sample` only speeds convergence.

### Case 4. A look with true shadows (171 tokens), and a deliberate exception (69 tokens)

```ts
const k = wgsl`
fn shade(x: Point, wo: vec3f) -> vec3f {                       // radiance leaving x toward wo
  let m = material(x);
  let s = light(0, x.p);                                       // emitter 0 is the sky's sun
  let lit = visible(x.p, s.p).r * max(dot(x.n, s.wi), 0);      // true shadows, from every shape and medium
  let tone = 0.25 + 0.35 * step(0.05, lit) + 0.4 * step(0.5, lit);
  return albedo(m) * s.L * tone;
}`;

export default scene({ things, sky: sky(SUN), camera, engine: print({ shade: k.shade }) });
```

A look is an estimator: this one replaces only what light does at a point. Now a woodblock wants the washer's hole
closed **in the shadow only** (P's test of who owns a decision):

```ts
const solid = world(things.map((t) => (t === ring ? thing(disc(1), brass, at([0, 0, 1])) : t)));
// in shade:  let lit = ${solid.visible}(x.p, s.p).r * max(dot(x.n, s.wi), 0);
```

The camera, mirrors and picks keep the scene's world. `loft show` prints which world each `visible` reads.

### Case 5. A whole engine: the reference path tracer (377 tokens)

```ts
const sum = target("rgba32float");
const k = wgsl`
fn path(px: vec2u, s: u32) -> vec4f {                          // one path through pixel px, sample s
  var r = key(px, s);                                          // same numbers whatever order paths run in
  var ray = camera_ray(vec2f(px) + next2(&r));
  var L = vec3f(0); var T = vec3f(1); var seen = true;
  for (var bounce = 0u; bounce < 16u; bounce++) {
    let x = trace(ray);
    if (x.t < 0) { L += T * sky(ray.d); break; }
    let m = material(x);
    if (seen) { L += T * emit(m, -ray.d); }                    // met by chance: counted only where no light was sampled
    let l = light(pick(&r), x.p);
    L += T * eval(m, -ray.d, l.wi) * l.L * visible(x.p, l.p) / l.pdf;
    let sc = sample(m, -ray.d, &r);
    T *= sc.weight; seen = sc.delta;
    ray = Ray(x.p, sc.wi, 1e-4, 1e30);
  }
  return vec4f(L, 1);
}
fn resolve(px: vec2u) -> vec4f { let s = load(${sum}, px); return vec4f(film(s.rgb / s.a), 1); }`;

export const reference = () => [
  screen(k.path, { into: sum, blend: "add" }),                 // each run adds one sample per pixel
  screen(k.resolve, { into: view }),
];
```

The judge of light uses every contract and schedules nothing. Two corrections over F's version come from A and P: a
discrete lobe is a tag (`delta`), not a density of zero, and every random number comes from a key of (pixel, sample,
bounce, dimension), so queue order cannot change the picture. The shipped engine adds media and multiple importance
sampling in about 60 lines.

### Case 6. Own the frame (107 tokens)

```ts
export const inspect = () => [screen(wgsl`
fn pixel(px: vec2u) -> vec4f {
  let x = trace(camera_ray(vec2f(px) + 0.5));
  if (x.t < 0) { return vec4f(0.18, 0.18, 0.18, 1); }
  return vec4f(0.5 * x.n + 0.5, 1);
}`.pixel, { into: view })];
```

No materials, lights, history or post. A shape with no `hit` and no adapter is a typed refusal at `trace` naming the
shape and the missing reading (A and P's case 5).

### Case 7. The default engine, whole (179 tokens)

```ts
export const realtime = (o = {}) => (s: Scene, f: Frame) => [
  ...s.lights.filter(mapped).map((l) => find(s, { from: l.view, into: l.map })),   // a shadow map is find from a light
  find(s, { into: vis, samples: 4 }),                          // one draw per cover; kinds dispatch inside
  screen(shadePixel(s, o.shade ?? direct, f.samples), { into: radiance }),
  screen(inscatter(s), { into: radiance, blend: "over" }),
  ...(f.history ? [accumulate(radiance, vis)] : []),           // the viewer's default; stills and clips never
  ...(o.post ?? [bloom, film]).map((p) => p(radiance, view)),
];
```

`radiance` is linear and scene-referred until `film`. `print` is this list with a look as `shade` and an ink pass.

### What a stranger asked, and what changed

- *"Which light is 0?"* Emitters are ordered: the sky's sun first, then glowing things by power. A rule, not a name.
- *"Why does a flat washer have a thick bound?"* The `boxes` cover rasterises the bound; a box with no volume covers
  nothing edge-on. A flat bound is refused by name.
- *"Do I need `world`?"* Only when one class of ray should see a different scene.
- *"What does `samples: 4` do?"* It sets coverage samples (edges), not light samples (§3.3).
- P's `studio([torus(1, 0.2, brass)])` (34 tokens) was rejected: it says nothing a scene with defaults cannot, so the
  name buys no capability.

## 2. Three designs, one ruling

All three agree to: define a shape once for every observer; replace paint returning rgba with a material that can be
evaluated and sampled; compose media in linear radiance with one transmittance; keep the frame scene-referred until
one display transform; make module count independent of instance count; judge light by a path-traced reference;
declare history; move ground, outline, medium policy and the sun out of the Rust core.

| Question | F | A | P | Ruling and reason |
|---|---|---|---|---|
| What the author defines | A kind: struct plus readings | Solids with interval semantics; accelerators as candidates | `surface(bound, hit, material)` | **F.** Constructor arguments are rows, so 1,000 washers are one module. P's `surface` is a kind with one reading; A's interval solids become CSG kinds from a `span` reading (Phase 5) |
| Primary visibility | Record (thing, where, depth), one shade pass | Forward, attachments priced | Forward, fused; a record costs 16 B a sample | **F, with 4× coverage.** Shading in each cover compiles the shade code once per cover and runs it once per overlapping fragment; from the record it compiles once and runs once a pixel. The record's traffic equals wire 4's multisampled colour target (§5.3). `inspect` still shades forward |
| Render–light seam | Six global queries | Compile per request (first hit, attenuation, coverage) | `nearest`, `beam`, a world per ray class | **F's queries over P's worlds.** Worlds buy deliberate disagreement (case 4). A's projection is how `visible` is written: any-hit reading only `t`, so dead-code elimination drops normals and charts |
| Material | pdf 0 marks a mirror | Discrete and continuous distinct | Same as A | **F plus a `delta` tag.** A tag is a sum type; a magic zero is a silent case |
| Media | One in-scatter pass, scalar `tau` | Joint integration; red-over-blue test | `(L, T)` per segment, RGB | **All three**: one pass over summed coefficients, `(L₁ + T₁L₂, T₁T₂)`, RGB depth, A's test in Phase 3 |
| History | Body-rate `cached`; screen history keyed by the record | Charge preroll; score frames 1, 4, 16 | A texture pair; history in the identity | **Lead decision 3 with F's mechanisms.** `cached` lowers to pixel rate without history because replay costs more than samples (§5.3). A's scoring and P's identity rules are adopted |
| Code count, tiers | One module a pass; lifetime tier rule; GPU tape | Offline tuning per device | Measured selector; schedule in the identity | **F's rule, recomputed** (§5.5). The tape waits for Phase 4: the baseline shows dispatch count, not compile, hurting frames, and fused compiled fills fix that |
| Substitution | Adapters derive readings | Candidates with conditions | Equivalence at the observer; the folded coral's 172 pixels | **P's rule over F's adapters.** An adapter states which observations it reproduces exactly |
| Phase 1 | A Rust fixture | The bead through the import; a tile experiment | The bead and a washer with four observers | **Ordinary import, F's scene and checks, P's oracle, A's bore rays.** F avoided TS because the library recorded no draws; wire 4 records them. A's tile classifier waits: A predicts it loses on the bead |

What only one source had: from **F**, the three questions, open functions with a generated switch, holes as fields,
one estimator under removable schedule statements, the body rate and the tier rule; from **A**, the axial-bore
counterexample, integer identity (an f32 copy index aliases above 2²⁴), the coincident-media numbers, shutter bounds
and the reverse-Z host contract; from **P**, worlds as values, equivalence at the observer, a per-pixel explanation
trace and the washer oracle, whose counts I re-derived (9,664, 12,080, 12,892).

## 3. The architecture

### 3.1 Layers

| Layer | Holds | Lives in |
|---|---|---|
| Core (meta engine) | Functions, tables, kinds, passes, rates; compiler, runtime, twin, checks, solve | `rust/crates/loft-{ir,core,gpu}`, `src/record`, `src/text` |
| Render sub meta engine | `Ray`, `Hit`, `Box`, `Point`; shape readings, adapters, covers, `find`, the record, worlds | `src/render/` |
| Light sub meta engine | `Scatter`, `Lit`, `Sigma`; the six queries, emitters, sample keys, history, in-scatter | `src/light/` |
| Library | Shapes, materials, lights, skies, media, looks, post | `src/kinds/`, `src/looks/` |
| Example engines | `realtime`, `print`, `reference`, `inspect` | `src/engines/` |

### 3.2 The core: five nouns

| Noun | What it is | Today |
|---|---|---|
| function | WGSL text with holes, lowered to the graph; a call stays a call | `Op::Call`, `GraphFn` |
| table | Rows or a texture, with a fill law and a clock | Kernels, packs, `Keep` |
| kind | A struct, the table of its instances and its functions. An **open function** is a reading several kinds define, called with a kind chosen by data | **New** |
| pass | One GPU program run: compute over an extent, or raster draws into targets, at a clock | Fixed forms; generalised, and `Kernel` becomes a compute pass |
| rate | The binders a value reads: time, thing, texel, vertex, pixel, sample | Two bits (`rate.rs`); extended |

WGSL has no function pointers, so a choice among laws is a `switch` that the compiler writes. An open call whose kind
is known is a direct call; otherwise it calls a generated function whose cases are the kinds present in this
composition. Kind ids come from content hashes, so a module's text depends on the set of laws only. **Holes are
fields**: a number in a hole or a constructor becomes a row value; literals inside a WGSL body are law.

The Rust types, in `loft-ir`. They replace wire 4's `Draw`, `Cover`, `RasterPrim`, `Find`, `Proxy`, `Compose`,
`Medium`, `Integrate`, `Target` and `Kernel`; `When`, `Keep`, `Camera`, `Resource`, `Pack`, `Module`, `Bundle`,
`Identity`, the check, the verdict and the error stay.

```rust
pub struct Kind  { name: NameId, fields: Box<[Field]>, rows: ResId }       // a struct; its instances are rows
pub struct Open  { name: NameId, sig: Sig, cases: Box<[(KindId, FnId)]> }   // a reading and the kinds that define it
pub enum   Op    { /* today's, and */ Field(u16), Make(KindId), Open(OpenId) }
pub enum   Ty    { /* today's, and */ U32, Kind(KindId) }
pub struct Pass  { run: Run, reads: Box<[Read]>, writes: Box<[Write]>, when: When, repeat: u32 }
pub enum   Run   { Compute { entry: Entry, extent: Extent }, Raster { draws: Box<[Draw]>, camera: Id, samples: u8 } }
pub struct Draw  { vertex: Entry, fragment: Entry, vertices: u32, instances: Count, raster: Raster, layer: Layer }
pub enum   Count { Fixed(u32), Rows(ResId), Indirect(ResId) }
pub enum   Read  { Now(ResId), Previous(ResId) }                            // Previous: last frame's texture of a pair
pub struct Write { target: ResId, blend: Blend, clear: Clear }
```

### 3.3 Render: readings, covers, the visibility record

The interface text every `wgsl` module is read with is library text, and it prints:

```wgsl
struct Ray     { o: vec3f, d: vec3f, t0: f32, t1: f32, w: vec2f }   // w: footprint radius w.x + w.y·t; 0 off-camera
struct Hit     { t: f32, n: vec3f, uv: vec2f }                     // the kind's frame; t < 0 a miss; n outward
struct Box     { lo: vec3f, hi: vec3f }
struct Point   { p: vec3f, n: vec3f, uv: vec2f, t: f32, thing: u32 }
struct Scatter { wi: vec3f, weight: vec3f, pdf: f32, delta: bool } // weight = f·cos/pdf; delta: a discrete lobe
struct Lit     { wi: vec3f, L: vec3f, pdf: f32, p: vec3f }         // pdf in solid angle at the receiver
struct Sigma   { s: vec3f, t: vec3f }                              // scattering and extinction per metre, RGB
```

A ray starting inside a solid meets the exit first, with the solid's outward normal; A's bore rays test that.

| Role | Readings | Signatures |
|---|---|---|
| shape | `hit`, `bound`, `dist`, `place`, `gap` | `(s, Ray) -> Hit`, `(s) -> Box`, `(s, vec3f) -> f32`, `(s, vec2f) -> vec3f`, `(s, Ray) -> f32` (how near a missing ray passes: ink and edge coverage) |
| surface | `eval`, `sample`, `emit`, `albedo` | `(m, wo, wi) -> vec3f`, `(m, wo, &key) -> Scatter`, `(m, wo) -> vec3f`, `(m) -> vec3f` |
| medium | `sigma`, `tau` | `(m, vec3f) -> Sigma`, `(m, Ray) -> vec3f` |
| light | `light` | `(l, vec3f) -> Lit` |

**Adapters** derive a missing reading and state what they reproduce exactly:

| Has → wants | Adapter | Exact for | Made true by |
|---|---|---|---|
| `dist` → `hit` | `march` inside the bound, normal from the symbolic gradient | hit, depth, normal | Intervals bound the slope, else `Unproved` by name |
| `dist` → `hit`, dear | `bricks`, baked at load in body coordinates | hit within a declared tolerance | Scored against the formula |
| `place` → `hit` | `patches`, the grid's box tree refitted by fills | the drawn triangles | — |
| `hit` → pixels | the `boxes` cover from `bound` | coverage, depth, `gap` ink | The twin finds no hit outside the bound |
| `eval` → `sample`, `albedo` | cosine-weighted; `π · eval(z, z)` | any continuous lobe; a matte lobe | The twin; declared approximate otherwise |

A missing reading with no adapter is a refusal naming both.

**Covers** are wire 4's: `boxes`, `grids`, `meshes`, `quads`, `screen`, reorganised so that a cover is one pipeline
per pass with kinds dispatched by instance. A cover's fragment runs only the shape: `hit` and `gap` for `boxes`,
interpolants for `grids`.

**The visibility record**, per coverage sample, is one `rgba16uint` and the depth: 12 bytes.

- Lanes 0–1: the thing's row (31 bits) and an ink bit. Lanes 2–3: *where*. For a grid that is (u, v) at the pixel
  centre as two unorm16, for a mesh the triangle index, for a ray-found shape nothing.
- `rgba16uint` because in wgpu 30.0.1's guaranteed format table it is multisample-capable and `rg32uint` and
  `r32uint` are not (`wgpu-types-30.0.1/src/texture/format.rs`, the `msaa` column) `[repo]`.
- Four samples with centre values, because R0 was drawn 4× multisampled with each fragment evaluated once at the
  pixel centre and ray-found shapes covering samples through a mask from `gap`. Wire 4 measured the alternative: a
  crab seed's silhouettes were 6,341 pixels over the rule until it matched R0's sampling, then 3 (`9c9b9f1`)
  `[repo]`. Centre values written to each covered sample reproduce R0's coverage by construction.
- The shade pass runs once a pixel, reads four samples and shades each distinct thing once at the pixel centre,
  weighted by its count: once inside a shape, twice on an edge. Ray-found shapes refine from the recorded depth, not
  a fresh march. Grids evaluate `place` and its symbolic gradient at (u, v), so point and normal are exact. Meshes
  recompute barycentrics at the centre (Burns and Hunt).

`samples: 4` is coverage. Light samples are counted in the shade pass. That is how this design reads the lead's
"1 sample plus history": one light sample a pixel a frame, edges already resolved by coverage.

**Worlds.** `world(things, accel?)` gives `trace` and `visible`. `flat` loops over bounds and pays up to about 30
things (§5.3); `groups` uses the authoring hierarchy as the tree, refitted with one fill per level; a sorted tree and a
hardware ray query are later backends for meshes behind the same `hit`.

### 3.4 Light: queries, emitters, media, history

| Query | Answers | Built from |
|---|---|---|
| `trace(ray) -> Point` | The nearest surface | Every shape kind's `hit` through the world's `accel` |
| `visible(a, b) -> vec3f` | Transmittance from a to b | Any-hit over `hit`, then `tau` or a march |
| `material(x)` | The response at x | The thing's response kind |
| `light(i, p) -> Lit` | A sample of emitter i toward p; `pick(&key)` draws one by power | Light kinds; glowing things by area, converted to solid angle |
| `sky(d) -> vec3f` | Radiance from direction d | The scene's sky, a closed form |
| `medium(p) -> Sigma` | Coefficients at p, summed over media | `sigma` |

There is no light list and no cap. Transparent surfaces are responses that transmit, met by rays; there is no sorted
alpha. One in-scatter pass walks each camera ray to the record's depth through the summed media, so overlapping
media integrate jointly.

**History is declared.** In the viewer, `accumulate` keeps two view-sized `rgba16float` textures and the previous
sample-0 thing; a pixel blends where the thing matches and restarts where it does not. Phase 1 does this for a still
camera. Phase 3 adds F's exact reprojection: the same body point's pixel at t − dt comes from evaluating its pose and
chart at t − dt. Body-rate `cached` (F) is Phase 3 and viewer-only. Stills, clips and training take every sample
inside the frame.

### 3.5 How a composition compiles

TypeScript records functions, structs, tables, things, worlds and the pass list, keeping calls as calls and constants
as rows (the compact-graph lane). The compiler collects the kinds each pass reaches, derives missing readings or
refuses by name, lifts each value to the slowest pass that binds what it reads (build, load, frame, thing, texel,
vertex, pixel, sample), plans covers, adapters and rates (fixed rules in Phase 1, printed predictions in Phase 4),
emits one module per pass with its dispatchers, fuses kernels of one clock and dependency level into one compute
pass, and lays rows into the existing packs. The runtime writes the frame uniform and runs the passes in order on a
device the caller may own. To replace any part, the consumer writes a value, a kind, a function passed to an engine,
or a pass list (cases 2–7); nothing in that list belongs to a built-in.

## 4. Wire 4: adapted

The draws lane's result, now on main (`4e8ee7b`): of 295 scenes, **233 build, load and draw**; **13 pass every
frame**; **1,183 frames fail** the picture rule; **8 are refused** (offscreen passes 3, author compute 2, `loft_cast`
2, `soft_box` 1) `[repo]`. The frame failures fall into named classes: grain post passes (fixed after the run),
TS's inner-silhouette ink on ray-found solids not drawn (the bead), ink width at small features (the owl),
biolume's reflection pass reading `sea_height` (138 frames), and 161 frames from unported drawables (quads, Gaussian
media, copies' per-kind hit, glazed surfaces, impostors).

Each class has a place in this design, and that decides it. The refusals all need textures readable by draws and a
raw kernel scheduled per pass: `Pass` with `reads` and `Run::Compute` is exactly that. Inner-silhouette ink is the
record's ink bit written from `gap`. A post pass that reads `sea_height` is the core assuming its content; the sea
becomes a kind. The unported drawables are Phase 2's kinds. And the module counts track fill kernels: the Titan's 990
modules are 889 compute pipelines and 116 render, the heron's 159 are 133 and 27, the lineup's 5,538 are 4,905 and 653
`[repo]`. The decision is to **adapt**: keep what is content-free, replace what multiplies with content.

| Wire 4 | Fate | Why |
|---|---|---|
| `Bundle`, `Resource`, `Pack`, `Module`, params, inputs, sites, `Identity`, `Camera` (a GPU `lens` entry) | Kept | Content-free; passes from a light and the view need exactly these cameras |
| `Cover::Raster{Grid, Mesh, Quads}`, `Cover::Ray{proxy}`, `Cover::Screen`; `Raster{cull, depth_write, blend}`; `Layer` | Kept as the five covers, one pipeline per cover per pass | They are F's covers already |
| 4× multisampling, nearest-sample depth resolve | Kept, moved into the record | The 6,341 → 3 pixel result |
| `src/draw/record.ts`, `Drawable.record`, `unported` answers, the runner's cost columns | Kept | Where things record; the instrument for every phase |
| `Draw.shade`, paint returning rgba in each cover's fragment | Replaced by the record and one shade pass | Shade code compiled once, run once a pixel |
| `hit(o, d, span, width, id: f32) -> vec4` | Adapted to `hit(s, Ray) -> Hit`, u32 rows | An f32 id aliases above 2²⁴ |
| `Compose { outline, ground }` | Deleted from the wire: ink is a record bit and a library pass; the ground is a plane kind | Policy in the core |
| `Medium`, `Integrate`, `Find::Medium`, veiling inferred from `depth_write` | Replaced by medium kinds and one in-scatter pass | A flag-inferred veil is privileged policy (P); author order is wrong for overlap (A) |
| `Target::Shadow { light, map }` | Replaced by a texture target and a raster pass with its own camera | A shadow map is `find` from a light |
| One module per draw; one module and one dispatch per kernel | Replaced by one module per pass and fused fills | §5.1 |

Nothing persisted depends on wire 4's draw declarations (R0 frames are PNGs), so changing them costs no migration.

## 5. Performance theory, to the number

All `[repo]` numbers are from the Apple M4 (Metal) the corpus is measured on, at 960 × 540 unless stated.

### 5.1 Phase 0, done: the measured baseline

| Scene | Draws | Frame kernels | Modules (render / compute) | WGSL lines | Load | GPU p50 / p95 ms |
|---|---:|---:|---|---:|---:|---|
| `bead` | 3 | 0 | 8 (4 / 5) | 1,597 | 16 ms | 0.44 / 2.34 |
| Owl | 18 | 59 | 91 (19 / 73) | 35,389 | 307 ms | 2.69 / 6.01 |
| heron | 26 | 131 | 159 (27 / 133) | 69,057 | 539 ms | 1.69 / 8.08 |
| Titan (reefback) | 116 | 872 | 990 (116 / 889) | 362,641 | 35.1 s | 6.79 / 7.20 |
| five-crab lineup | 653 | 4,880 | 5,538 (653 / 4,905) | 2,259,509 | 23.1 s | 97.2 / 109.1 |
| Median of 229 measured | | | 330 | 134,267 | 1.27 s | 4.16 / 6.11 |

Costs from the draws lane's run (`963c667`); lines, draws and kernels parsed from each `scene.loft` `[repo]`. The TS
engine draws the five crabs at 4.26 ms (1280 × 800) from 305 modules after a 54.9 s load (third-party audit).

- **Large scenes' frames follow commands.** Over all 229 scenes, p50 ≈ 1.30 ms + 15.3 µs × (draws + frame kernels)
  + 33 ms per million grid cells at the finest level, R² 0.87 `[fit]`. The lineup leverages that fit: without it the
  slope is 17.7 µs and R² 0.66, and below 500 commands other costs dominate (R² 0.26). Per scene with at least 100
  commands, (p50 − 1.3 ms) ÷ commands has median 19 µs (quartiles 4 and 26); the Titan pays at most 5.6 µs a command
  `[fit]`. For the lineup the fit gives 84.7 ms of commands and 15.8 ms of finest-level cells against 97.2 ms
  measured. Its frame kernels average 3.9 rows each (19,116 rows over 4,880) `[repo]`: a pipeline switch and a
  dispatch for almost no work. The cells term is wire 4 issuing every grid at its finest level and clipping unused
  vertices in the vertex stage.
- **Load follows lines, and new code is ten times dearer.** The median scene loads at 9.2 µs a WGSL line (quartiles
  8.6 and 10.9); the slowest tenth, whose shaders the driver had not seen, at 92 to 137 µs `[repo]`. A model's fresh
  scene pays the second. A linear fit on modules and lines explains little (R² 0.18) because cache state dominates.
- **Build CPU is over the cap.** Median 3.15 s; 192 of 229 over the 2 s training cap; the lineup 52 s `[repo]`.

### 5.2 Constants

| | Value | Band | Source |
|---|---|---|---|
| d, one GPU command | 15.3 µs | 5 to 40 | `[fit]`; F had 10 (8.5 to 33) |
| κw, a WGSL line the driver has seen | 9.2 µs | 8.6 to 10.9 | `[repo]` |
| κc, a line it has not | 110 µs | 92 to 137 | `[repo]` |
| c, a scalar operation; ℓ, a coherent load | 2 ps; 30 ps | 1–4; 10–90 | `[conjecture]`, F |
| τb, a bound test; τh, an analytic hit | 50 ps; 0.2 ns | | `[derived]`, F |
| B, effective attachment bandwidth | 100 GB/s | 60 to 120 | `[conjecture]`; M4 peak 120 |
| ρ, interpreted over compiled | 30 | 10 to 60 | `[conjecture]`, F |

Phase 1's `tools/calib` measures c, ℓ and d. A constant outside its band rewrites this section before a dependent
check is scored.

### 5.3 Frame

**The record** at 1280 × 800: 1,024,000 pixels × 4 samples × 12 bytes = 49.2 MB stored; with depth discarded after
find, 32.8 MB, read once more by the shade pass. About 66 MB, **0.66 ms** at B `[derived]`. Wire 4 already stores 4
samples of colour and depth (32 bytes a pixel) and reads them to resolve: the same traffic. At 512², 0.17 ms.

**A ray** over a flat list of n things with h hits costs n·τb + h·τh: 1.9 ns for n = 30, h = 2. Over `groups`, 658
things as 26 groups of 25 with two entered, 4.2 ns (F) `[derived]`. Through grid surfaces' patch trees about 14 ns
`[conjecture]`, so the crabs take sun shadows from a map found from the sun, a declared approximation scored against
the reference.

**The shade pass** over a flat list at 1280 × 800 with one bounce costs 1.9 ms + 0.11 ms × N (F): derive the surface
0.2 ns, material 0.2, the sun's `visible` 1.9, `eval` 0.3, the bounce's `trace` 1.9, half the bounces landing 1.2,
`sample` 0.2, per pixel `[derived]`. Edges add about 3%.

| Scene | Commands | Find | Shadows | Shade | Rest | p50 |
|---|---|---|---|---|---|---|
| Slice, N = 10, 1280 × 800, 1 light sample | 7 | 0.5 + 0.66 record | rays | 3.0 | 0.25 | **4.5 ms** |
| Slice, N = 30 | 7 | 0.5 + 0.66 | rays | 5.2 | 0.25 | **6.7 ms** |
| Slice, 512², 16 light samples, no history | 6 | 0.13 + 0.17 | rays | 16 × 1.33 | 0.03 | **21.7 ms** |
| Five crabs, `print`, 1280 × 800 (Phase 2) | ≤ 30 (0.46) | 1.5 + 0.66 | 0.8, map | 1.3 | 0.5 | **5.2 ms** |
| Five crabs, `realtime`, cached bounce (Phase 3) | ≤ 34 | 1.5 + 0.66 | 0.8 | 2.5 | 0.5 | **6.0 ms** |

The five-crab find is F's 1.5 ms conjecture for the draw stage once paint leaves the fragment (today's TS draw stage
is 3.47 ms with it) and grids draw their chosen level through compacted indirect counts. The lineup's 97 ms falls to
about 5 ms by removing about 5,500 commands and drawing chosen levels instead of the finest `[derived]`.

**Why the no-history path does not use `cached`.** A cold frame replaying a 16-step body cache costs about 24 ms for
the five crabs (F's 1.2 ms a step). The same bounce at pixel rate costs 3.2 ns a pixel: 0.84 ms a sample at 512²,
13.4 ms for 16 samples, with no state `[derived]`. So stills, clips and training lower `cached` to pixel rate.

### 5.4 Load and module count

Load ≈ setup + Σ over modules of lines × (κc for new text, κw for seen) + a cost per pipeline. The module set is the
engine's: covers for the view and each light's map, shade, in-scatter, history, post, one fused fill per clock and
level. It does not count things.

| Scene | Pipelines | WGSL lines | Warm | Cold | Baseline |
|---|---|---|---|---|---|
| Slice, N = 10 to 1,000 | ≤ 8, the same for every N | ≈ 1,800 | **0.03 s** | **0.28 s** | — |
| Owl, heron (Phase 2) | ≤ 24 | ≤ 8,000 | 0.08 s | ≤ 0.9 s | 307 ms, 91 modules; 539 ms, 159 |
| Titan (Phase 2) | ≤ 30 | ≤ 15,000 | 0.15 s | ≤ 1.7 s | 35.1 s, 990 modules |
| Five crabs (Phase 2) | ≤ 30 | ≤ 20,000 | **0.2 s** | **≤ 2.0 s** | 23.1 s, 5,538 |
| All measured scenes, one run | | | **≤ 60 s** total | | 964 s total |

The slice: 1,800 lines × 9.2 µs ≈ 17 ms warm, × 110 µs ≈ 0.2 s cold, plus pipelines `[derived]`. The corpus total is
`[conjecture]`: after the first scene of each kind set, the engine's modules are text the driver has seen. Line
ceilings for the creatures depend on the compact graph. Wire 4's lineup kernels are structurally distinct per
occurrence: 4,884 kernel modules reduce only to 4,640 distinct texts with every numeric literal masked `[repo]`,
because the recorder inlines. Fusing them without the compact graph would cut commands but build million-line
modules, so a fused module is capped at 20,000 lines until the recorder keeps calls. Adding a shape kind changes the
modules that dispatch shapes (`boxes`, shade, `reference`'s path): about 3 × 600 lines × 110 µs ≈ 0.2 s cold.

Memory at 1280 × 800: record 32, depth 16, radiance 8 and view 4 bytes a pixel (61 MB); the viewer's history adds 20
bytes a pixel; a thing's rows are 112 bytes (74 KB for 658 things); a 2048² shadow map is 16.8 MB.

### 5.5 When to interpret, share or specialise

A site of L lines runs n operations an invocation, P invocations a frame, for F frames. Compiled it costs κL +
F·P·n·c; interpreted, F·P·n·c·ρ. At two operations a line interpreting wins when F·P < κ / (2c(ρ − 1)): **9.5 × 10⁵
runs** for new text, 7.9 × 10⁴ for seen text `[derived]`. Specialising a shared site saves Δn operations for a
pipeline (b ≈ 5 ms `[conjecture]`) and ΔL lines. For Δn = 10 and ΔL = 30 of new text it pays after (5 + 3.3) ms ÷
20 ps = 4.2 × 10⁸ runs: 68 s of a site covering a tenth of a 1280 × 800 frame at 60 Hz, while a whole 16-frame 512²
clip at 16 samples is 6.7 × 10⁷ `[derived]`. **Loft never specialises for a clip; the viewer specialises a site hot
for over a minute.** F's conclusion survives recalibration. Unconditional specialisation is where 54.9 s (TS) and
23.1 s (wire 4) went.

### 5.6 What would change the design

- Shade time over twice the sum of its kinds measured alone (switch register pressure): classify tiles by response
  class, one shade dispatch per class.
- Record traffic over 1.5 ms at 1280 × 800: the viewer drops to one coverage sample with jitter in the history;
  parity runs keep 4.
- A fused or shade module over 20,000 lines, or driver time superlinear in lines: split by law group.
- Find over twice its prediction because proxies write depth from the fragment: find dear kinds in compute, seeded
  by raster depth (F).
- ρ over 100: drop the tape tier (F). A cached term over 5% from the reference outside a texel of contact: gather at
  pixel rate with screen history (F).

## 6. How each thing is judged

**Shape, silhouette and depth: picture parity against R0 on a light-free look (R0g).** Light changes on purpose, so
R0's beauty frames cannot judge geometry. R0g is R0's commit (`d3674db`) drawn again with one added look: each thing
a flat colour from a hash of its draw label, ink as authored, no light, plus a depth image (8-bit log distance). Same
cameras, times and sizes; the lean plan's rule (a) unchanged. It is built once on a branch from R0. The new engines
draw it with `print({ look: flat })`. A pixel can differ only if coverage, ink or depth differs, so the gate is
independent of light and of where shading runs.

**Light: against a converged `reference`,** converged by independent seed batches, doubling samples until the 95%
confidence half-width of each scored linear patch is at most 0.005, or 4,096 samples (A). It scores closed forms
(furnace box, white furnace, dye slab, coincident media); `realtime` without history against `reference` at equal
bounce cap (mean relative bias ≤ 1% on interior pixels, where all four record samples agree); each schedule statement
(bounce cap, shadow map, `cached`, a multiple-scattering fit, a sample count) removed one at a time, with its change
in error printed (F's "one estimator, two schedules" as a report); the viewer's error at equal time, with the first
16 frames after a cut scored apart (A). For print looks, the inputs a look reads (`visible` per light, irradiance) are
scored against the reference's; the look's mapping to colour is art, judged by eye and the aesthetic reward.

**Determinism.** Without history a frame is a function of the scene, t, the sample count and the keys: two runs give
the same bytes, and a frame drawn alone equals it drawn inside a clip. The identity record gains the engine, light
sample count and history flag, and in the viewer the frames since reset.

## 7. Inherited and ours

| Inherited | From |
|---|---|
| Visibility buffer, barycentrics recomputed at the pixel; edge-only extra shading under MSAA | Burns and Hunt 2013, Nanite; Lauritzen 2010 |
| Shape, BSDF, light, medium, integrator | pbrt |
| Interfaces resolved by a generated switch; one material code for reference and real time | Slang, Falcor |
| Rates moved without editing the term; algorithm apart from schedule | RenderMan, Spire 2016; Halide |
| Shading on the object's own parameters | Reyes, texel shading, Lumen's surface cache |
| Rasterise the bound, intersect inside; GPU-compacted indirect draws | Teardown, Loft; Wihlidal 2016 |
| Frame as data with declared reads and writes | Frostbite FrameGraph |
| Resampled importance sampling; GGX energy compensation; counter-based random numbers | Talbot, Bitterli; Kulla and Conty; Salmon et al. 2011 |
| Tape in a fixed kernel; interval CSG and Lipschitz pruning | Keeter 2020; Duff 1992, Barbier et al. 2025 |

| Ours | From |
|---|---|
| Consumer laws with a program count fixed by the engine: kinds, open functions and holes as fields over a record of any `hit` | F |
| The real-time picture as the reference estimator under removable, scored schedule statements; the body rate; replayable history; the lifetime tier rule | F |
| Worlds as values per class of ray; equivalence at the observer; a per-pixel explanation trace | P |
| Observation-specific compilation (any-hit by projection; interval tile classification as research) | A |
| A cost model of wire 4 fitted from its parity run; kernels as passes, fused by clock and level through the kinds' switch | this note |
| A multisampled `rgba16uint` record with centre values that reproduces R0 coverage by construction, and R0g, a geometry gate independent of light | this note |

## 8. Phased plan

Budgets are lines added (or removed) per layer. "Core" counts the Rust crates and the TS recorder and text front end.
Scenes, oracles and tools are counted apart and are not product. Phase 0 is done (§5.1).

### Phase 1. The vertical slice

**Branch.** From main at `4e8ee7b`:
`git cambium add -b feat/render-slice -name render-slice -ephemeral ~/git/_wt/loft-render-slice`. One implementer.

**Build.**

- **Core.** `loft-ir/src/{graph,bundle,wire}.rs`: `Ty::U32`, `Ty::Kind`, kind and open declarations,
  `Op::{Field, Make, Open}`; `Pass`, `Run`, the new `Draw`, `Count`, `Read`, `Write`, `TexFormat::{Rgba16Uint,
  Rgba32Float}`, sample counts, as wire 5 beside wire 4's draws. `loft-core/src/open.rs` (new): kind rows in the
  packs, one switch per open reading over the kinds present. `loft-core/src/pass.rs` (new): one module per pass.
  `loft-gpu/src/pass.rs` (new): an ordered pass list with several targets, 4× targets, additive blend, `repeat`, and
  previous-frame reads from a texture pair. `src/record/decls.ts`, `src/text/`: record structs, readings, worlds, pass
  lists.
- **Render** (`src/render/`): `interface.wgsl`; `covers/boxes.wgsl` (with the `gap` mask and ink bit) and
  `covers/screen.wgsl`; `find.ts`; `record.ts`; `world.ts` (`flat`, `trace`, any-hit `visible`); the `eval` adapters.
- **Light** (`src/light/`): `interface.wgsl`; `queries.ts` (`medium` returns zero); `emitters.ts`; `key.wgsl`;
  `history.ts` (still camera).
- **Library** (`src/kinds/`): plane, sphere, box, capsule, washer, bead (from `examples/bead.ts`, bore rays fixed);
  matte, metal (GGX with Kulla–Conty, a 32 × 32 albedo table filled at load), glow, lamp (a matte that glows); a
  closed-form sky with a sun; `film`; the `flat` look.
- **Example engines** (`src/engines/`): `realtime`, `print` (toon, gap ink, `flat`), `reference`, `inspect`.
- **Scenes and instruments.** `examples/slice/`: the slice (a plane, four capsules, two boxes, the bead, one glowing
  sphere, spheres up to N, a sun and a sky); the washer oracle (washer ID 7 at z = 1, card ID 9 at z = 0, an
  orthographic camera at z = 3 over [−4, 4]² at 512², one centre sample, light from the card toward (2, 0, 1)); the
  closed furnace box; the white furnace. `tools/slice_oracle.ts`, `tools/calib`, a cold-load nonce in
  `loft-draw profile` (a literal that survives translation). R0g for the bead only.

| Layer | Budget | Owner files |
|---|---|---|
| Core, Rust | +900 | `loft-ir/src/{graph,bundle,wire}.rs`, `loft-core/src/{open,pass}.rs`, `loft-gpu/src/{pass,lib}.rs` |
| Core, TS recorder and text | +250 | `src/record/decls.ts`, `src/text/` |
| Render | +450 | `src/render/` |
| Light | +350 | `src/light/` |
| Library | +600 | `src/kinds/` |
| Example engines | +250 | `src/engines/` |
| Scenes, oracles, tools | +500, not product | `examples/slice/`, `tools/` |

**Relation to the Rust port.** Additive on wire 4: the corpus keeps drawing through wire 4's draws, and a scene opts
in by naming an engine. `loft-loader` is untouched; `loft-node` bumps its ABI. Both draw paths coexist on main only
until Phase 2's cutover.

| # | Check | Predicted | Passes if |
|---|---|---|---|
| 1 | Pipelines and module texts, slice at N = 10, 100, 1,000 | ≤ 8 pipelines, ≈ 1,800 lines | Hashes identical across N; ≤ 2,000 lines |
| 2 | Edits | Washer r0 0.5 → 0.25: 0 module texts change, 1 row. Adding the bead kind: 3 texts change (`boxes`, shade, `reference`) | Exactly |
| 3 | Load after the device exists | Warm 0.03 s, cold 0.28 s | Warm ≤ 0.06 s, cold ≤ 0.35 s, spread ≤ 10% across N |
| 4 | Washer primary IDs | 9,664 samples of ID 7; 12,080 after the edit | Exact |
| 5 | Card shadow mask | The plate mask shifted 128 pixels left: 9,664, then 12,080. With case 4's `solid` world: 12,892 blocked, primary still 9,664 | Zero differing bits |
| 6 | Picks and secondary rays | Picks at (0, 0) and (0, 0.375): ID 9 at t = 3; after the edit the second is ID 7 at t = 2. Rays from (0, 0, 2) and (0, 0.375, 2) toward −z: t = 2, 2, then 2, 1 | Exact IDs; t within 1e-4 |
| 7 | Bore rays on the bead (A), twin and GPU | (0, 4, 0) down: miss. (0.6, 4, 0) down: t = 2.2. (0, 1, 0) along +x: t = 0.35, n = −x. (0.6, 1, 0) along +x: t = 0.4, n = +x | Within 1e-5 m |
| 8 | Record against `trace` of the centre ray, two cameras | 0 interior pixels differ; depth within 1e-4 relative | ≤ 0.01% of pixels differ |
| 9 | Closed furnace box, `reference`, albedo 0.5, walls emit E | E(1 − 0.5^(D+1))/0.5: 1.50E, 1.75E, 2.00E at D = 1, 2, 16 | Within 1% at 1,024 samples |
| 10 | White furnace: white metal, roughness 1, uniform sky 0.5 | 0.50 at centre and rim (today's lobe: 0.15 at the centre, albedo 0.307) | Within 2% |
| 11 | `realtime` without history against `reference`, one bounce, 256 samples | Equal in expectation | Mean relative bias ≤ 1%, interior pixels |
| 12 | Shade pass at 1280 × 800, N = 10, 30 | 3.0, 5.2 ms; whole frame 4.5, 6.7 ms p50 | Slope and intercept within 30% after calibration; frame within 2× |
| 13 | Training: 512², 16 frames, 16 light samples, N = 30 | 21.7 ms a frame | p95 ≤ 50 ms |
| 14 | Determinism and history | A still at t = 1.37 alone and after drawing 0 to 1.37; viewer, still camera, 64 frames against a 64-sample still | Same bytes; mean relative difference ≤ 1% |
| 15 | R0g on the bead, 9 frames | 9/9 (the axial fix touches at most the axis pixel) | Rule (a) |
| 16 | Core words | 0 rendering identifiers in core code this phase adds | 0 |
| 17 | Tokens of cases 2 to 6 as built | 263, 231, 171 + 69, 377, 107 | Within 15% |

**Stop rule.** Two failed fixes in one subsystem stop the phase; the lane note says which prediction missed and what
the theory got wrong before anything is retried.

### Phase 2. The corpus on kinds, and the cutover

**Build.** Grids indexed, each copy's chosen level compacted into one indirect draw per cover (`Count::Indirect`);
meshes by triangle index; quads as `place(id, corner) → world, uv, fade` (the cut audit's signature); `march`,
`bricks`, relief's own root finder (its residual is not a safe step), `patches`, `groups`. The 13 drawables become
kinds (strand, sheet, lathe give `place`; field gives `dist`; impostors, traced, copies give `hit`; specks, glow,
rain are quads; density and volume are media); the sea is a kind over a projected-grid cover; the ground is a plane.
Gap ink and hull outlines write record ink. The TS styles become `print` looks. Shadow maps are `find` from a light.
Fused fills. Textures readable by draws and scheduled author compute, which lower wire 4's 8 refusals. R0g for the
corpus.

**Delete.** Wire 4's `Draw`, `Cover`, `Find`, `Proxy`, `Compose`, `Medium`, `Integrate`, `Target::Shadow`, `Kernel`;
`loft-core/src/draw/mod.rs`; the 13 templates (their text moves to `src/render/covers/`); composition and resolve
policy in `loft-gpu/src/view.rs`. The TS engine and `loft-loader` go at the lean plan's cutover.

**Depends on** the compact-graph lane for line counts (§5.4). Without it the command and module predictions hold and
the line and cold-load ones do not.

| Layer | Budget | Owner files |
|---|---|---|
| Core | net ≤ −500: about −900 (draw lowering 390, templates 242, view composition about 250), +400 (fused fills, indirect counts, texture reads) | `loft-core/src/{draw,kernel,schedule,pass}.rs`, `loft-gpu/src/{view,lib}.rs` |
| Render | +900 | `src/render/` |
| Light | +250 | `src/light/` |
| Library | `src/draw`, `src/style`, `src/soft` (11,560 lines today) become `src/kinds`, `src/looks` at ≤ 9,000 | `src/kinds/`, `src/looks/`, `src/geom/sea.ts` |
| Example engines | +250 | `src/engines/` |

**Relation to the Rust port.** The lean plan's wave-3 cutover with the draw side changed: one bundle path, one
executor, the loader retired once its level-of-detail, camera and target capabilities exist in `loft-gpu`.

| Check | Predicted | Passes if |
|---|---|---|
| R0g, all frames of every built scene | First full run ≥ 90% pass; the rest named defects | 100% before cutover, deliberate corrections listed |
| Refusals | Wire 4's 8 lowered; verdicts and typed refusals as R0 | Lean plan (b), (c) |
| Five crabs | ≤ 30 commands (5,533), ≤ 30 pipelines (5,558), ≤ 20,000 lines (2,259,509); p50 5.2 ms at 1280 × 800 | p95 ≤ 6 ms; load warm ≤ 0.3 s, cold ≤ 2 s (23.1 s); build ≤ 5 s (52 s) |
| Titan, heron, Owl | ≤ 30 pipelines each (990, 159, 91) | Load warm ≤ 0.2 s (35.1 s, 539 ms, 307 ms); p95 at or under 7.20, 8.08, 6.01 ms |
| Corpus | Median p95 3.0 ms (6.11); median modules ≤ 32 (330) | Every scene's p95 at or under its baseline; total load ≤ 60 s (964 s); training seeds build ≤ 2 s |
| Sun map against reference visibility | ≤ 3% of pixels differ (2.72% between `castRay` and the lamp map, DESIGN 17.7) | Reported; over 5% names the scene |
| Core | 0 rendering identifiers in `loft-{ir,core,gpu}`; `loft-gpu` ≤ 1,600 lines | Both; per-frame encoder ≤ 500 lines |

### Phase 3. Light

**Build.** Clear coat, sheen, transmission, thin film (stated RGB fit and illuminant), anisotropy; subsurface as a
random walk in `reference` and a declared fit in `realtime`. Resampled importance sampling over emitters at a fixed
number of shadow rays a pixel. `(L, T)` media and the in-scatter pass. `cached` with `Keep::Window`, viewer only.
Exact reprojection for the history. Exposure, bloom on a mip chain, depth of field and motion blur by samples, grain,
grade. Light checks (`litBy`) through `visible`; furnace and sampling verdicts for a consumer's BSDF from the twin.

| Layer | Budget | Owner files |
|---|---|---|
| Core | ≤ +150 (`Keep::Window`, mip views) | `loft-ir/src/bundle.rs`, `loft-gpu/src/pass.rs` |
| Render / light | +200 / +900 | `src/render/`, `src/light/` |
| Library / example engines | +1,500 / +200 | `src/kinds/`, `src/looks/`, `src/engines/` |

| Check | Predicted | Passes if |
|---|---|---|
| Coincident red and blue media, σt = ln 2 each, unit source in its channel, over black | (0.375, 0, 0.375), transmittance 0.25, in either order | Within `Tol::f32_rel(1e-4)` |
| Absorbing dye slab, 1 m, (0.2, 0.7, 1.4) per metre | (0.81873, 0.49659, 0.24660); any split, same product | Same |
| 1,000 glowing spheres at ≤ 4 shadow rays a pixel | Unbiased | Bias ≤ 1% against `reference` |
| `cached` bounce | Within 5% of reference outside a texel of contact | As predicted |
| Five crabs, `realtime`, cached bounce | 6.0 ms p50 at 1280 × 800 | p95 ≤ 7 ms |
| Every shipped BSDF | Furnace and χ² sampling tests | All pass |

**Relation to the Rust port.** `Keep::Window` in `loft-ir`, mip views in `loft-gpu`; nothing else.

### Phase 4. Tiers and the plan

**Build.** The GPU tape kernel (one fixed module, always cached) for fills under §5.5's threshold; background compile
and swap in the viewer; the planner's printed predictions (`loft show --plan`) and a calibration file per device;
`loft show --pixel x,y` (P's bounded trace: candidates, spans, the accepted thing, source sites, transmittance,
densities); p95 in `loft profile`. Budgets: core +500 (`loft-core/src/{tape,schedule}.rs`, `loft-gpu`, `loft-draw`),
render and light +200. Checks: ρ ≤ 60 or the tier is dropped; a fresh lantern-sized scene's first frame within 0.1 s
of the device; planner within 30% of profile on the corpus median.

### Phase 5. Scale and embedding

**Build.** A sorted tree for meshes, built at load and refitted; a hardware ray query behind the mesh kind's `hit`
where native backends expose it, chosen by A's break-even (an update cost U over a per-ray saving s pays after U/s
rays); glTF in as kinds, with alpha as coverage every observer sees; CSG kinds from a `span` reading (A); for the
coral, per-observation residuals and A's interval tile classification where the per-sample law costs 6 ns or more;
Keel, with a subject found and shaded offscreen and composited with the game's depth, and the host's depth
convention (reverse Z included) in the camera. Budgets: core ≤ +200 (an acceleration-structure resource behind a
capability, `loft-ir`, `loft-gpu`), render +1,200, library +800. Checks: a tree over 100,000 triangles in ≤ 0.1 s;
the coral's close view ≤ 6 ms at 1280 × 800 with fold clipping intact and R0g passing (25.89 ms marched today; 3.08
ms listed with 1.5–9.5% of pixels different); Keel with zero caller clears, forced submits and synchronous
readbacks. This is the lean plan's wave 4.

**Totals.** Core +1,150 (Phase 1), net −500 (Phase 2), +850 over Phases 3 to 5. The sub meta engines reach about
4,000 lines, the library about 12,000, the example engines under 1,200. The meta engine ends smaller than wire 4's
draw side and executor, and everything above it is public code a consumer can copy.

## 9. For the owner

1. **R0g as the geometry gate**: one look on a branch from `d3674db` and one more corpus render. Without it every
   change of light reads as a regression in shape.
2. **Order Phase 2 after the compact graph.** Its command and module wins do not need it; its line and cold-load
   wins do, and fused fills over inlined kernels would build million-line modules.
3. **Accept the record's cost**: about 0.66 ms at 1280 × 800, the traffic of wire 4's multisampled colour target, for
   R0-exact coverage and shade code compiled once.
4. **Score the grain fix first.** The draws lane's next full run (at `3c99d1a` or later) is the baseline Phase 2's
   per-scene frame and parity checks compare against; the numbers above are from `963c667`.


================================================================
# MOTION ARCHITECTURE
# file: docs/proposals/2026-11-19_motion_architecture.md
# what it is: our consolidated motion design (evidence, not instructions): body/state/pose, ends, pins, laws, fused ticks
================================================================

# Motion architecture: one machinery for adaptive locomotion and performance (19 Nov 2026)

Status: proposal. Nothing here is built. It consolidates three designs into one:

- **F**: Fable, `agent_notes/2026-11-19_design/motion_fable_step{1,2}.md` and `motion_fable_phase1b.py`.
- **A**: Astra, `motion_astra_step{1,2}.md`.
- **P**: Astra with the full philosophy, `philosophy_pass/motion_step{1,2}.md`.

Evidence: the audit (`agent_notes/2026-11-19_audits/motion_audit_astra.md`), the look rounds
(`agent_notes/2026-11-19_motion.md` on `feat/motion`) and the Rust draws note (`agent_notes/2026-11-19_lean_rust_draws.md`).

My reading of the task: one motion sub meta engine whose example engines give both Spore-like adaptive locomotion
and Disney-style performance. The owner liked the S-neck of constant length, the level head with a glance capped at
15°, and feet that step because the weight asks them to; those become defaults. GPU frame time decides every runtime
choice.

## 0. The answer on one page

1. **The core gains no motion concept.** It uses the five nouns the render design already names (function, table,
   kind, pass, rate) and gains six small runtime facts that a fluid or a flock would need too (§2.6).
2. **The motion sub meta engine** is three table seams (BODY, STATE, POSE), four end kinds (`foot`, `head`, `hand`,
   `loose`), a library of laws written in WGSL, and three TypeScript names: `body`, `move` and `clip`.
3. **An engine is one WGSL module with two functions**: `step` advances a tick, `present` evaluates the pose at a
   fraction of one. The schedule is the order of calls in `step`. Loft ships two example engines, `roam` (adaptive
   locomotion) and `act` (performance: `roam` with one law replaced and two added), ordinary files anyone can copy.
4. **All motion is one dispatch per frame.** Each invocation advances its creature by the ticks it owes, presents its
   pose and measures its holds on the drawn sites. The source designs each spent three. On the Rust path today the
   five-crab lineup spends 97 ms a frame across 4,905 per-creature compute pipelines. With motion as rows and draws
   per kind, it should fall to about 24 pipelines and at most 4.3 ms (§8.1).
5. **Truth.** Every hold is measured on the drawn site, in its support's frame, naming what gave way and the trace.

| The owner asked for | Where it comes from |
|---|---|
| Spore-like locomotion that adapts at once to body, player and reversal | `roam`: no cycles; feet step from capture-point strain once their load is low; legs and neighbours come from the body |
| Disney-style performance | `act`: beats with authority, anticipation from the boundary conditions of a plan, follow-through driven by actual acceleration, squash from load, stillness by withholding motion |
| The neck and the gaze-fixed heron | The `head` kind's defaults: an S of two arcs of fixed length, a level skull, a glance capped at 15°, hold then thrust |

## 1. The consumer's code first

Tokens are `o200k_base` on each block, imports included. `shell`, `titan` and `human` are defined elsewhere;
`input`, `wgsl`, `scene` and `time` exist today.

**1. A crab that scrambles under a player's stick.**

```ts
import { body, move, foot, roam } from "loft/motion"

const legs = [[1.35, 2.8, 6.6], [0.5, 1.1, 7.4], [-0.4, -0.9, 7.3], [-1.3, -2.7, 6.4]]
const crab = body({
  trunk: { size: [4.8, 1.05, 5], height: 2.5 },
  chains: [-1, 1].flatMap((s) => legs.map(([x, fx, fz]) =>
    ({ root: [x, 0.1, 2.5 * s], lengths: [3, 2.9, 1.2], end: foot([fx, 0, fz * s]) }))),
})
const scuttle = move(crab, roam(), { go: input("stick", "vec2") })

export default scene({ things: shell(scuttle), ground: plane, cameras })
export const checks = scuttle.holds
```

**2. A heron that tracks a fish while hopping.** The neck has a range. When the fish leaves that range the body
turns, but only while a foot is down. In flight nothing can turn the body.

```ts
import { body, move, foot, head, roam, hop } from "loft/motion"

const heron = body({
  trunk: { size: [1.6, 0.6, 0.5], height: 1.9 },
  chains: [
    ...[-1, 1].map((s) => ({ root: [-0.1, -0.2, 0.18 * s], lengths: [1.05, 1.1], end: foot([0, 0, 0.18 * s]) })),
    { root: [0.5, 0.2, 0], lengths: [1.55], end: head([0.5, 1.18, 0], { turn: [1.9, 0.6], glance: 0.26 }) },
  ],
})
const bird = move(heron, roam({ lift: hop }), { go: input("stick", "vec2"), look: input("fish", "vec3") })
```

`bird.holds` reports in this form (1.58 rad is derived in §4.7; the times are illustrative):

```text
gaze   held      0 over the take until 3.10 s; neck settled at 1.58 rad with the fish circling at 1 rad/s
gaze   gave way  from 3.10 s for 0.21 s, up to 0.27 rad: neck at its 1.90 rad range in flight, no foot down to turn the body
neck   held      length error at most 0.2 mm on the drawn S
```

**3. A boss hit in the middle of an attack.** The claw keeps its beat, the trunk recoils and may step, the antennae
whip because the trunk moved, and the face stays still because nothing loose hangs from it.

```ts
import { move, act } from "loft/motion"
import { titan } from "./titan"          // six feet, two claws (hands), a head, two antennae (loose)

const slam = { clawR: [[0.7, [4, 0, 1.2], [0, -20, 0]], [1.3]] }   // be there at 0.7 s moving 20 m/s down; let go at 1.3 s
const boss = move(titan, act(slam), {
  at: input("at", "vec3"),           // the game owns where the boss stands
  since: input("attack"),            // seconds since the game began this attack
  hits: input("hits", "events"),     // point and impulse in world space, each applied once, on its own tick
})
```

No key asks for a wind-up: starting at rest 3 m above its mark and arriving at 20 m/s, the claw first rises 0.69 m
(§4.6).

**4. Motion from outside in, poses out.**

```ts
import { move, act, clip } from "loft/motion"

const walk = clip("walk.glb", human, { LeftFoot: "footL", RightFoot: "footR", Head: "head", Hips: "trunk" })
const walker = move(human, act(walk), { since: time })
export const checks = walker.holds   // where the take gave way to the body: site, ticks, metres
export const pose = walker.pose      // joint frames each frame: drawn, read back, or written by `loft export --gltf`

const live = clip(input("generated", "pose"), human, map)   // a model run elsewhere writes one source pose per tick
```

**5. Replace one law.** This heron starts turning its body at half its neck's range.

```ts
const early = wgsl`
fn turn(c: Creature, b: Body, k: u32) -> f32 {   // yaw rate head k asks of the trunk
  let neck = bearing(c, b, k);                    // the head's turn from the trunk, unwrapped, radians
  let ease = 0.5 * b.chain[k].turn.x;             // start turning the body at half the neck's range
  return sign(neck) * max(abs(neck) - ease, 0.0) / b.tc;
}`
const bird = move(heron, roam({ lift: hop, turn: early.turn }), { go: stick, look: fish })
```

A body a model invents needs no motion code. F's five-legged star with a head on a stalk is the crab's shape of
file with other numbers: 222 tokens, 9 lines.

**6. Replace the whole engine.** The body drifts with its limbs hanging, in the manner of Rain World.

```ts
const drift = wgsl`
struct Creature { at: vec3f, vel: vec3f }
fn step(c: Creature, b: Body, in: Inputs) -> Creature {
  let vel = c.vel + ((vec3f(in.go.x, 0, in.go.y) - c.vel) * 4.0 + vec3f(0, -9.81, 0)) * DT;
  var at = c.at + vel * DT;
  at.y = max(at.y, b.height);
  return Creature(at, vel);
}
fn present(c: Creature, b: Body, f: f32, s: u32) -> Frame {   // every segment hangs at rest under the moving trunk
  return Frame(c.at + c.vel * f * DT + b.rest[s].t, b.rest[s].q);
}`
const drifter = move(crab, drift, { go: input("stick", "vec2") })
```

| Case | Tokens | Lines | The same thing today |
|---|---|---|---|
| 1 Crab scramble: body, motion, scene | 235 | 10 | 2,434 tokens for the look round's path, stepper and scene (F), with no player |
| 2 Heron, fish and hop | 221 | 9 | 3,957 tokens for the gaze round's two files (F), on a fixed clock, with no fish |
| 3 Boss hit in mid-attack | 185 | 8 | Cannot be written: `recur` refuses host inputs |
| 4 Clip in, retarget, poses out, live generator | 151 | 6 | No import exists |
| 5 One law replaced | 144 | 7 | Edit the scene's expressions |
| 6 Engine replaced | 200 | 12 | Not applicable |

A reader learns three verbs (`body`, `move`, `clip`), four kinds (`foot`, `head`, `hand`, `loose`) and two example
engines (`roam`, `act`); `hop` is one of `roam`'s laws, exported so it can be named. The seams' WGSL structs are
printed by `loft context` from the code.

## 2. Architecture

```text
engines                roam · act · a consumer's own              WGSL modules; examples; any size
motion sub meta engine BODY · STATE · POSE · end kinds · laws     body(), move(), clip(); library written to be rewritten
core                   function · table · kind · pass · rate      knows nothing of feet; compiler, runtime, twin, checks, solve
```

### 2.1 Three seams

```text
build:  body plan ──► BODY rows (per plan)            score or clip ──► BEATS rows
frame:  for each creature, in ONE dispatch:
          load STATE; for n in owed ticks: STATE = step(STATE, BODY, inputs[n]); store STATE
          POSE = present(STATE, BODY, fraction); margins of every hold on the drawn sites ──► POSE
        POSE ──► draws (vertex stage) · a host that reads poses back · `loft check`
```

- **BODY**: segments with rest frames, twist and joint limits; sites (a pad point, an eye ray, a palm frame); one row
  per chain with its end kind; the trunk's derived numbers (mass, inertia, h, T_c, R, μ, strength); neighbour edges.
- **STATE**: one record per creature, a WGSL struct the engine owns, built from library parts (trunk, a pin per end, a
  row per loose chain, a plan). It holds the **span** of its current tick (start values and the acceleration chosen),
  so `present` can evaluate the pose anywhere inside the tick.
- **POSE**: one rigid frame per segment (32 B: translation and quaternion) and a margin row per held site. It feeds
  everything that draws, exports or checks.

Each seam is a plain table with a documented layout. A host can write POSE from its own solver, and Loft draws and
checks it.

### 2.2 One dispatch, ticks fused

Every source design spent three dispatches a frame: two steps and one pose. A dispatch in a short burst costs 8.5
to 33 µs (DESIGN.md §14.1, correction 2), more than the few microseconds of arithmetic in a creature's tick.

A creature's step reads its own state, BODY and its inputs, never another creature's rows. So one invocation runs
every tick it owes in a loop, then presents, with no barrier between ticks. A static bound of 64 covers a 30 Hz frame
(4 ticks), a 50 ms stall (6) and a seek from a checkpoint (at most 59). State is read and written once per frame, not
per tick. The compiler sees from a kernel's reads whether it stays in its own rows; a flock's step, which reads
others, keeps a dispatch per tick. No author flag.

The idea is the audit's XPBD alternative (an island's ticks in one dispatch), made the default.

### 2.3 Present at a fraction, not a mix of two states

F drew a mix of the last two states, up to 8.3 ms behind. I take A's form, `present(S[k], t − t[k])`:

- The trunk is integrated exactly for constant acceleration over a tick:
  `x' = x + v·dt + ½a·dt²`, `v' = v + a·dt`.
- `present` evaluates `x + v·Δ + ½a·Δ²` at Δ ∈ [0, dt). At Δ = dt this equals the next state, so nothing pops at a
  tick boundary.
- Swings are closed forms in their share s, and plans are closed forms in time.
- The attitude is interpolated between the tick's two ends.

The presented tick's decision used only inputs up to its start, so no response comes before its cause.

### 2.4 Three homes, one function

| Home | When | Mechanism | What it gives |
|---|---|---|---|
| Closed | A part has full authority from a score and reads no state (a keyed hand) | An expression of time inside `present` | Interval proofs; any frame alone |
| Baked | Every input the step reads is a script (an input whose default is an expression of time) | The same `advance` entry with `Keep::Every`, run once at load | Scrubbing; training's 16 frames |
| Live | A host writes an input | `Keep::Latest`, a per-tick input log, checkpoints | Player control; seek and replay from the log |

`move` chooses the home from what the step reads, and `loft show` prints it. An instance stays baked until its host
writes an input, then continues live from the baked state at that tick (F's "one continues the other"). Live and baked
call one compiled entry, `advance(from, to, keep)`, because Metal may contract `a*b+c` differently in two entries and
bit equality is a phase 1 gate.

### 2.5 Kinds of end

The end of a chain is a **kind** in the render design's sense: a WGSL struct plus the functions written for it
(`strain`, `repin`, `swing`, `present`). The library ships four:

| Kind | What it holds | Its re-pin when the slack runs out | Default giver |
|---|---|---|---|
| `foot` | A point in a support's frame | A step | trunk |
| `head` | Head place, skull attitude and eye ray, separately | A thrust of the head, then a turn of the body | gaze |
| `hand` | A place and arrival velocity from a score | The score's next beat | tip (`gives: "body"` makes a lunge) |
| `loose` | Nothing; it follows its root with weight | None | none |

A consumer adds a kind (a sucker that holds walls, a wheel) by writing one struct and its functions. The compiler
writes the dispatcher over the kinds present; with one kind there is no switch. Until the render lane lands open
functions, `move` writes the switch; the phase 1 crab needs none. The other sum types, each with one dispatcher, are
support (`Flight | Point | Segment | Polygon`), pin mode (`Planted | Swinging | Scored`), input
(`Level | Script | Events`) and home (`Closed | Baked | Live`).

### 2.6 What the core gains

| # | Change | Where | Why |
|---|---|---|---|
| 1 | Inputs are a sum type: `Level`, `Script(expr of time)`, `Events { slots }`. Each instance keeps a per-tick log ring, and an event is consumed once, by sequence number | `loft-ir` `InputDecl`, `loft-gpu` | Catch-up ticks now replay today's input on yesterday's ticks; a hit held over six of them is six hits |
| 2 | An instance owns its live state, tick cursor and checkpoints (every 60 ticks) | `loft-gpu` `instance()`, `prepare` | `instance()` refuses `Keep::Latest`, so reactive creatures cannot share a pass |
| 3 | Fused ticks: a row-local step runs `advance(from, to)` in one dispatch (static bound 64) | `loft-core` kernel and schedule, `loft-gpu` encode | One dispatch per frame for all motion (§2.2) |
| 4 | Steps run before the frame kernels and draws that read them | `loft-gpu` `encode_into` | Frame kernels are encoded before live steps, so readers see the last frame's pose |
| 5 | `Scope::Trace { ticks, inputs: digest, by: Gpu \| Twin }` | `loft-core` check | A verdict on stepped state. Today it is `Undecided(State)` |
| 6 | Frame identity includes the checkpoint, the input-prefix digest, the tick and the fraction | `loft-ir` identity | Two histories that end with the same stick draw different crabs, so their keys must differ |

None of the six names a foot or a gait; a fluid or a flock needs the same six. The grep gate
`rg -i -w 'foot|gait|creature|trunk' rust/crates` stays empty, as it is today.

F's records in rows are not needed: the library's WGSL loads and stores its structs from vec4 rows. `body()` runs
at record time in TypeScript, as the scene's own arithmetic, so Rust receives only rows and modules.

## 3. Where the designs differ, and what this design takes

| Question | F | A | P | Taken, and why |
|---|---|---|---|---|
| What an engine is | A WGSL module between table seams | Composable verbs (`walk`, `track`, `recover`, …) | Plain functions in order | **F's seams; P's schedule as plain calls in `step`.** One module per engine keeps cost independent of creature count and kind. A's verbs survive as laws, not names |
| Public names | 7 | About 11 | About 13 | **3 verbs, 4 kinds, 2 engines.** A law is replaced by its WGSL name in a slot (`roam({ turn })`) |
| Dispatches per frame | 3 | 3 | 3 | **1, ticks fused** (§2.2): the largest frame-time lever here |
| When the pose is drawn | A mix of the last two states, up to a tick behind | The state's span at the fraction | — | **A**: no lag, no pop, half the reads (§2.3) |
| Contact and load | Least-norm shares; can go negative | LP extremes: a share interval | Hull margin without a foot | **Each where strongest** (§4.2): hull for feasibility, nonnegative least-norm shares for the visible load (continuous, unlike LP extremes), the interval as a query |
| Who may lift | Jacobi, ties by index | After a balance witness | One decision per snapshot | **Greedy in strain order inside the invocation.** Jacobi lets two feet each lift assuming the other carries, and F's staggered first lifts (feet 0, 2, 5 at 1.033, 1.058, 1.092 s) come from chain order, not the body. Here load staggers them (§4.3) |
| Where a hold is true | Residuals from `step` | Drawn pad, deck space | The observed site | **P**: margins on the drawn site after IK; F's residuals stay as diagnostics; P's false plant is the gate |
| Gaze | Head pin; neck strain turns the body | Eye ray, skull and head place separate; unwrap | Each independently editable | **F's pin carrying A's three quantities**, with the unwrap. In flight no foot can turn the body |
| Body description | Chains | Segments, sites, surfaces | Segments, sites | **F's chains over A and P's segment and site table**, which keeps twist, rest frames and limits |
| Directed motion at build | Shooting through the step | Eliminate flight, solve near keys | Keys as preference | **Rust `solve` near hand keys** (panel c, owner-approved) with A's elimination; shooting is a phase 5 experiment |
| Authority | A number per part | One writer per degree of freedom | One writer, by order | **One rule**: one writer per degree of freedom per tick; F's authority is the default blend, in WGSL |
| Secondary motion | Exact per-tick spring on the root's acceleration | — | — | **F**; the audit's closed-form spans become a phase 5 lowering |
| Parallel layout | Invocation per creature | Workgroup per creature | Invocation | **Invocation**: a crab's serial work is about one dispatch (§8); A's layout is a lowering for heroes |
| Phase 1 | Crab; picture on TS | Weight transfer, four pads, Rust draw | Moving deck, swing edit, false plant | **The crab, live and drawn by Rust**, with P's false plant and edit and A's history identity; four pads and decks in phase 2 |

## 4. The default laws

Units are metres, seconds and radians. h is the trunk's height, T_c = √(h/g), and R is the strain radius, 0.18 of
the leg's span.

### 4.1 The pin (F)

A planted foot and a held gaze are one constraint: an end fixed in the world, a chain as slack, and a re-pin when the
slack runs out.

- **Hold.** A planted pin's point is copied from tick to tick, so its slide in state is exactly 0.
- **Strain** is the slack used over the slack available. For a foot it is
  |p − (home + v·T_c)| / R, the distance from where the capture point says the foot should be. For a head it is the
  neck's turn over its range.
- **Strain flows to the root; pose flows to the tip.** A strained neck asks the trunk to turn, which moves the homes,
  which strains the feet, which step.

### 4.2 Support and load

- The **support** is a sum type over the planted feet: `Flight | Point | Segment | Polygon`. A polygon is the convex
  hull of up to 16 points, built by monotone chain. Collinear and coincident sets fall into `Segment` and `Point`.
- **The clamp.** Holding acceleration a needs the pressure point p\* = c − (h/g)·a (the linear inverted pendulum). If
  p\* lies outside the support, project it to the nearest point q′ and take a′ = (g/h)(c − q′) (A). Then
  |a′| ≤ μg, no pull (vertical a ≥ −g) and push ≤ strength·g. In `Flight` the only force is gravity, so a flight is a
  parabola by construction.
- **Loads.** w = argmin ‖w − w_pref‖² subject to Σw = 1, Σw·p = p\* and w ≥ 0. With w_pref = 1/n and no active
  bound this is F's closed form, w_i = 1/n + (p_i − p̄)ᵀS⁻¹(p\* − p̄); a negative entry drops out and the form is
  solved again (at most n passes, usually one). A strictly convex QP's answer is continuous in p\*. An author who wants
  one foot to look loaded shifts w_pref. A's `shareRange(i)` returns foot i's feasible interval: [0.25, 0.50] in A's
  four-pad case (pads at (±0.30, ±0.20) m, q = (0.15, 0)), where the least-norm answer is 0.375.

### 4.3 Who lifts

A foot lifts when all of these hold:

- it is planted;
- its strain exceeds 1 and it has been down 0.12 s;
- no Cruse neighbour is swinging or was chosen this tick;
- its load share is at most 1.1/n;
- the remaining planted feet still contain p\* with an inset of 0.25 m.

Candidates are taken greedily in order of strain from the tick's starting snapshot. Neighbour edges (adjacent feet
along each side, plus the mirror partner) are derived from geometry by an editable library function, so five or six
legs need no index arithmetic. The load cap makes the stagger physical: under sideways acceleration p\* moves up to
1.5 m toward the trailing side, so the leading feet unload first. Evaluated (§10), the first four still lift together
from the symmetric start and the reversal breaks into staggered pairs, with swings of 0.120 to 0.334 s; a pure strain
rank gives only 0.21 to 0.27 s.

### 4.4 Swing (F, with round 4's shaping by need)

The swing is p = from + (aim − from)·m(s) + up·lift·16s²(1 − s)², with the least-jerk profile
m(s) = 10s³ − 15s⁴ + 6s⁵.

- The aim follows the body until s = 0.5 and is committed by s = 0.85.
- Duration and lift are set at lift-off from the step's length and urgency (round 4).
- The foot lands with zero vertical speed and under 0.08 m/s horizontally; today's half sine arrives at π·lift/T.

### 4.5 Trunk

- The chosen horizontal acceleration is a = (go − v)/τ with τ = T_c/2, so a larger animal answers more slowly.
- After the clamp, the trunk is integrated exactly (§2.3).
- **Lean** is a spring (2 Hz, ζ = 0.7) toward atan(a/g).
- **Leash.** A planted chain longer than 0.97 of its length moves the trunk back. The leg is never longer than it
  is, and the foot never slides.
- **Hit.** An impulse J at point r on its tick gives Δv = J/m and Δω = I⁻¹((r − c) × J). The capture point jumps by
  Δv·T_c. Feet step if that passes R, so a body absorbs hits below R/T_c without stepping. For the crab that is
  2.57 m/s. A second hit adds to the state the first one left.
- **Yaw** is clamped by the friction torque the planted feet can give. It is zero in flight, where spin is conserved.

### 4.6 Plans: anticipation is a boundary condition (F)

A move with a deadline is stated by where it must arrive, at what velocity and when: (D, v₁, T). The least-jerk
quintic from rest is

  p(s) = D(10s³ − 15s⁴ + 6s⁵) + V(−4s³ + 7s⁴ − 3s⁵),  V = v₁T, s = t/T.

Its s³ coefficient is 10D − 4V, so **the motion goes backward first exactly when V > 2.5D**. Nobody asks for the
wind-up.

- **The boss.** The slam (D = −3.0 m, v₁ = −20 m/s, T = 0.7 s) gives p = 26s³ − 53s⁴ + 24s⁵, which rises 0.69 m at
  s = 0.522 (0.37 s).
- **A jump.** With D = 0 the lowest acceleration is −1.949 v₁/T. Feet cannot pull, so the stance T = 1.949 v₁/g, the
  crouch is 0.385 v₁²/g at two thirds of the stance, and the peak net push is 2.02 g.
- **The heron's hop** (v₁ = 3.876 m/s) gives a 0.770 s stance and a 0.590 m crouch at 0.513 s. The keys asked for
  0.60 m at 0.35 s, which needs the feet to pull: free fall alone takes 0.35 s to drop 0.60 m and arrives still
  moving.
- **Strength.** A body weaker than 2.02 g takes T = 3.94 v₁/(strength·g) and crouches deeper. One number per body
  sets the timing of every jump it makes.

Plans store end conditions and a start tick, evaluate in closed form, and replan only on a hit, a new aim or a clamp
(F: replanning every tick loses 5% of the take-off speed).

### 4.7 Head, neck and gaze

These are the defaults the owner approved, made causal.

- **Head place.** The head is held in the world. When its offset from rest passes 0.15 of the neck's length, it
  thrusts back with a critically damped follower: ω = 2π·2 Hz, with velocity feed-forward 1/ω (round 4's
  best-measured lead). A strain trigger replaces round 4's fixed 0.2 s clock, so the bob rate is v/(0.15L): 2.6 Hz
  at 0.6 m/s for a 1.55 m neck.
- **Skull and eye.** The skull is level; its pitch toward a target is capped by `glance` (15°), and it yaws toward
  the target within the neck's range. The eye ray is the skull's forward direction.
- **Unwrap.** The head's yaw is unwrapped and remembers its side, so a fish crossing behind cannot flip a saturated
  head between its stops in one tick (A found a 0.115° target change giving a 57.3° jump on `main`).
- **Turn.** The trunk's yaw rate is the stick's plus `turn`: sign(neck)·max(|neck| − 0.6·range, 0)/T_c. With a fish
  circling at 1 rad/s the neck settles at 0.6·1.9 + 1·0.44 = 1.58 rad, inside 1.9. The body can keep up to
  1.73 rad/s. Past that, or in flight, the gaze gives way and says so.
- **Neck.** `present` draws the neck as round 4's S of two circular arcs of fixed length L, with
  sin θ/θ = d/L solved by four Newton steps. A chord longer than L is a reported failure, not a clamp.

### 4.8 Loose chains and authority

- **Loose chains** are damped springs driven by the root's actual acceleration, stepped by an exact 2×2 transition
  computed at build, stable at any stiffness. The rate comes from length and gravity (`pendulum.ts`) unless `hz` is
  given, so follow-through is caused by what happened, hits included.
- **Authority** is the score's writer blended with the law's writer by an explicit function, one per degree of
  freedom.
  - The default for the trunk is 1 − shock, with shock = e^(−(t − t_hit)/T_c)·min(|Δv|·T_c/R, 1).
  - Ends with a beat keep authority 1.
  - Deliberate stillness, as in A and P's face held through an antenna's swing, is authority 1 with nothing loose
    attached.

## 5. Two example engines

**`roam`** is §4's laws with `intend` reading `go` (a wanted velocity) or `at` (a place the game owns), plus `look`
and `hits`. It has no cycle, clip or gait table: it adapts to a body because every law reads BODY, and to a player
because strain reads the velocity now. Its slots: `lift`, `strain`, `swing`, `turn`, `lean`, `intend`.

**`act`** is `roam` with `intend` replaced by a score, plus `shaped` (squash from load, stretch from speed) and
`shock`.

- A score is beats per chain: `[time, place, arrival velocity]`, or `[time]` to let go.
- A pin with a beat arrives on the beat's tick exactly: a swing's arrival is a parameter, so "lands at 1.2 s" is
  true by construction for ends.
- Trunk beats are a prior followed inside the clamp, and `holds` says where they could not be: "trunk behind its keys
  from 0.25 to 0.35 s: a 0.60 m crouch by 0.35 s needs the feet to pull".
- A directed hop splits at take-off and landing. The flight is a parabola because no foot is down; the take-off
  velocity follows from the landing beat, v_y = (y₁ − y₀)/T + gT/2 (A), and the stance's plan from that.

| Disney principle | Where it comes from in `act` |
|---|---|
| Anticipation | The plan's boundary condition (§4.6), and load leaving a foot before it lifts |
| Slow in and out; arcs | Least-jerk swings and plans; swings about a fixed root; ballistic flight |
| Follow-through and overlap | Loose chains driven by the parent's actual acceleration |
| Squash and stretch | Load share and speed (`shaped`) |
| Timing | Exact at beats for ends; T_c and strength for the trunk |
| Exaggeration | Gains on impulse, strength and gravity, not on poses |
| Staging, holds | Authority 1 and nothing loose: a dead stop is a law like any other |
| Appeal, solid drawing | The author's |

## 6. Interop

**In.** `clip(source, body, map)` turns joint tracks into a score:

1. **Normalise at the edge**: metres, a right-handed y-up basis, continuous quaternion signs, twist kept. An
   unmapped required joint is a typed error naming the map entry.
2. **Retarget rotations.** For each mapped joint, q_t(t) = q_t,rest·B·(q_s,rest⁻¹·q_s(t))·B⁻¹, where B takes the
   source joint's rest basis to the target's (A). Scale the root's path by the ratio of hip heights.
3. **Ends become beats** in body-relative form (Spore), scaled by the ratio of chain lengths.
4. **Contacts are proposed.** A foot is labelled planted where its speed is under 0.15 m/s·scale and its height under
   0.02 m·scale for 3 samples. Labelled intervals become pins, so they are exact. A generator's own labels are
   proposals too: where they disagree, the report lists it.
5. **One writer per segment.** The engine owns the chains whose ends are pinned or beat. The clip writes every other
   segment, such as a spine or tail, directly.

Generated clips (MotionBricks, Kimodo, diffusion) take the same path. Only the corrected result in POSE is measured;
a clip is never reported as meeting a hold.

**In, live.** An input of type `"pose"` is a per-tick packet of source rotations and root. The step retargets it each
tick (about 1.5k operations for 33 joints). A missing tick holds the last pose and reports its age.

**Out.** POSE is the posed skeleton. A host may map it asynchronously; Keel's frame loop maps nothing.
`loft export --gltf` samples POSE:

- local TRS (parent⁻¹·child) and inverse binds from BODY's rest frames, so the export keeps twist;
- keys split at contact changes and hits;
- a check that interpolated keys at mid-sample times stay within 1 mm of POSE on every held site, or the span is
  subdivided.

The take's identity records the source hash, the map and the engine's hash.

## 7. Truth

- **A hold is a signed margin on a drawn site, in its support's frame, over a domain.** The plant margin is
  ε − ‖D(t)⁻¹·F(t)·p − a‖ (P): D is the support's transform, F the drawn segment's frame after IK, p the site and a
  the anchor. A deck moving at 0.3 m/s carries a correct foot 6 cm in 0.2 s, and the check rightly sees no error.
- **Where it is measured.** `present` writes the margins into POSE in the same dispatch that draws, so the check reads
  what the picture shows. The library also measures reach, joint limits, the neck's length on the drawn S, gaze error
  from the final eye ray, support margin, least share and timing at beats. A consumer adds a hold as a WGSL function
  of POSE that returns metres or radians.
- **Givers.** Each hold names what gave way (trunk, tip or release) and states it as a sentence: "gaze gave way
  0.27 rad for 0.21 s from 3.10 s: neck at its range, in flight".
- **Verdicts.** `Scope::Trace` says "held over 480 ticks of take `stick` (digest …), on the GPU". Interval proofs
  stay with closed homes; a law outside the `wgsl` subset gets a GPU trace or a named `Undecided`. A verdict carries
  the hash of every law it read, so replacing a law retires it (A, P).

## 8. Performance theory

GPU frame time comes first. The constants:

| Constant | Value | Band | Source |
|---|---|---|---|
| d, one dispatch in a frame's short burst | 10 µs | 8.5 to 33 | repo: DESIGN.md §14.1, correction 2 |
| The look round's eight-foot step, warm, including its dispatch | 6.5 µs | — | repo: 480 steps in 3.1 ms of GPU |
| Timestamp quantum | 0.065 ms | — | repo |
| One dependent scalar operation on an otherwise idle lane | 2 ns | 1 to 3 | conjecture |
| Throughput for branchy scalar WGSL across the GPU | 300 Gop/s | 150 to 500 | conjecture (F assumed 150) |
| Effective memory bandwidth for scattered rows | 60 GB/s | 30 to 100 | conjecture (F assumed 30) |
| One pipeline; one WGSL line through the driver | 25 ms; 60 µs | 5 to 45; 13 to 74 | the render design's constants |

**Work per crab**, counted from the laws:

- **Per tick, about 2,500 operations:** strain 240, hull 300, shares 360, lifts 400, four swings 400, trunk and lean
  220, leash 320, diagnostics 60.
- **`present`, about 2,400:** span 80, eight three-link IK at 130, 25 frames at 40, eight margins at 30.
- **Per frame at 60 Hz:** two ticks and one present, about 7,400 operations.
- **Bytes per frame:** STATE read and written once (2 × 672 B), POSE written (800 B of frames and 128 B of margins),
  and two input packets (64 B). That is about 2.3 KB. BODY (2.8 KB per distinct plan) is shared and cached.

**Model:** T(N) = d + max(L, N·w/F, N·m/B). L is one creature's serial time, w its operations, m its bytes, F the
throughput and B the bandwidth.

| Case | L | Throughput term | Memory term | Predicted motion time |
|---|---|---|---|---|
| 1 crab, 60 Hz | 7 to 22 µs | — | — | **17 to 32 µs** (cold dispatch: up to 55): below one quantum |
| 1 crab, 30 Hz (4 ticks) or a 6-tick stall | 12 to 52 µs | — | — | 22 to 62 µs, still one dispatch |
| 5 or 64 crabs | 7 to 22 µs | ≤ 3 µs | ≤ 5 µs | the same as one crab |
| 1,000 crabs | 7 to 22 µs | 15 to 49 µs | 23 to 78 µs | **33 to 88 µs** |
| 10,000 crabs | — | 0.15 to 0.49 ms | 0.23 to 0.78 ms | 0.25 to 0.8 ms |
| One heron (2 feet, head, S-neck) | about 3,800 operations | — | — | at most a crab's |
| One boss (6 feet, 2 hands, head, 2 loose chains) | about 11,000 operations | — | — | 22 to 43 µs |
| Seek: replay 59 ticks from a checkpoint | 0.15 to 0.44 ms | — | — | one dispatch |
| Bake of a 4 s scripted take (480 ticks) | 1.2 to 3.6 ms | — | — | one dispatch for every creature at once; today 3.1 ms took 480 dispatches |

The source designs predicted 0.02 ms (F), 99 µs (A) and 75 to 195 µs (P) for one crab, and 0.35 ms for 1,000 (F),
414 µs for 64 (A) and up to 0.8 ms for 64 (P). Fusing removes two dispatches and halves the state traffic.

**Memory.**

| Item | Size |
|---|---|
| Crab STATE (trunk span 6 lanes, eight pins of 4, diagnostics and cursor 4) | 42 lanes, 672 B; F's untrimmed record was 1,424 B |
| POSE | 928 B |
| Kept 4 s take | 481 × 672 B = 323 KB |
| Live seek window of W seconds | 2W checkpoints of 672 B plus 3,840·W B of input log: about 5.2 KB per second of window, per creature |
| 1,000 crabs with a 4 s window | 21 MB |

**Load.** Motion adds one module per engine, with two entries (`advance`, and `present` alone for baked scrubbing).
That is two pipelines, about 770 WGSL lines (library and `roam`). By the render design's formula, load is
0.05 s + 0.046 s ≈ **0.1 s**, whatever the number or kind of bodies: five crabs with different bodies differ only
in BODY rows.

**Build and training.** `body()` takes tens of microseconds of CPU. Recording and the Rust compile of the motion
module stay under 50 ms, against the 2 s cap. In training (512² × 16 frames, 50 ms a frame), a scripted take bakes
once in at most 5 ms, and each frame is one present-only dispatch.

### 8.1 The five-crab lineup on the Rust path

The lineup was measured on `main` at `4e8ee7b` (`agent_notes/2026-11-19_lean_rust_draws.md`, 960×540):

| | GPU p50 / p95 | Load | Modules | Pipelines (render / compute) |
|---|---|---|---|---|
| Lineup | 97.2 / 109.1 ms | 23.1 s | 5,538 | 653 / 4,905 |
| Titan | 6.79 / 7.20 ms | 35.1 s | 990 | 116 / 889 |

The TS engine draws it at 4.26 ms.

**Reading** (derived): 97.2 ms over 4,905 compute pipelines is 19.8 µs each, inside the measured 8.5 to 33 µs per
dispatch. The frame is consistent with being bound by dispatch count, not arithmetic. Each crab brings about 981 fill
kernels and 131 draw pipelines, because its pose and its draws are code per creature. Phase 1 first counts the
per-frame dispatches. If most of the 4,905 run only at load, this reading is wrong.

| | Today (Rust) | With POSE rows and per-kind draws | Who removes it |
|---|---|---|---|
| Pose and motion fills | Most of the 4,905 (counted in phase 1), one per kernel per crab | 1 module, 2 pipelines, 1 dispatch a frame for all five | this design |
| Draw pipelines | 653 | about 22, one per kind, the same for 1 or 5 crabs (render design §4.4) | the render lane |
| GPU frame p50 | 97.2 ms | dispatch overhead falls to at most 24 × 33 µs = 0.8 ms; motion ≤ 0.065 ms; the rest is draw work, which TS does in 4.26 ms. **At most 4.3 ms** | both |
| Load | 23.1 s | a + bP + κL with P ≈ 24: **about 1.5 s** (render's 1.45 s plus motion's 0.1 s) | both |

Neither half reaches these numbers alone. Motion turns every per-creature pose fill into rows of one table. The
render lane's rule that "holes are fields" turns the per-creature draws into one draw per kind. The Titan's
712-line rig, recoil and antenna whip become one `act` kernel and its rows: predicted about 25 pipelines and 1.5 s
of load, from 990 and 35.1 s.

**What would change these numbers.**

- **State spills** to memory per law, not per frame: the memory term grows up to six-fold and 1,000 crabs pass 0.3 ms.
  Then keep hot state in workgroup memory (A's layout, under 4 KiB) or split the record.
- **A hero creature** passes a quantum at two ticks: use A's workgroup-per-creature lowering.

## 9. Inherited and invented

**Inherited** (attributions recalled from the source designs): the linear inverted pendulum and capture point
(Kajita; Pratt) for T_c, strain and the pressure point; Raibert's three small laws for §4.5; least jerk (Flash and
Hogan) for swings and plans; Cruse's rules for neighbour inhibition; least-norm force distribution and feasible
contact wrenches (Caron et al.) for shares; position as primary state (Jakobsen, PBD) and exact linear springs; lockstep
netcode for the live home; Spore's ends by role, body-relative motion and grid of bodies × motions; the small leading
proxy of Rain World, Overgrowth and Euphoria; the pigeon's hold and thrust (Troje and Frost); spacetime constraints
(Witkin and Kass) as Loft's `solve`; rest-relative retargeting and glTF skinning; and Loft's own `recur`, `Steps`
kernels, `wgsl`, verdicts, typed refusals, the look rounds' stepper and neck, and render's kinds and rates.

**Invented in the source designs, kept:**

- F: the pin; chosen acceleration clamped by what the pins can give; three homes on one kernel; anticipation from
  boundary conditions, with its constants (2.5D, 1.949 v/g, 0.385 v²/g, 2.02 g, R/T_c).
- A: the load interval as a quantity a director can control; presenting a span at a fraction; one writer per degree
  of freedom.
- P: the observed site as the output of a construction, the input to its check and the root of a dependency cut; the
  false-plant gate; span compression of kept takes.

**This consolidation's own:**

- fused ticks with `present` in one dispatch;
- greedy lifting inside the invocation, gated by load share and support;
- one QP that unifies F's closed form and A's interval;
- end kinds as render kinds, so render and motion share one dispatch mechanism;
- one compiled entry for baked and live, chosen for bit equality;
- yaw tied to support, so flight explains a lost gaze.

Each ingredient is old; the combination, with truth, replaceability and frame time as the interface, is new.

## 10. Phased plan

Each phase is a lane (`tools/lane.sh`) in its own worktree. The TS engine stays frozen: TypeScript only records body
rows, WGSL text, input declarations and explicit draws (`src/record/decls.ts`). Runtime and compiler changes are Rust
in `rust/crates`, and the library and engines are WGSL text the Rust compiler consumes unchanged, so motion leaves no
TS runtime to port. Budgets are scc code lines per layer; more than 20% over needs a note in the lane.

### Phase 1: the crab lives (vertical slice, one implementer)

**Build.**

- Core changes 1 to 6 of §2.6; `body()`, `move()`, the `foot` kind, the laws of §4.1 to §4.5, and `roam` without
  head, plans, hits or loose chains.
- A crab scene drawn by Rust from explicit draws that read POSE: capsule legs, an ellipsoid shell, coloured toe tips,
  a marked ground. One draw per kind, instanced over POSE rows.
- A headless Rust harness that feeds the stick per tick through the log at 30, 60 and 120 Hz presentations.
- Before building: count the lineup's per-frame dispatches with `loft-draw profile`, to confirm §8.1's reading.

**Scene.** The look round's crab dimensions, 4 s, 120 ticks a second. The stick is +1.2 m/s from 0.2 to 1.7 s,
−2.1 m/s from 1.7 to 3.2 s, then 0. Other values: τ = T_c/2 = 0.252 s, μ = 0.6, R = 1.3, share cap 1.1/n, support
inset 0.25 m, swings shaped as in round 4.

**Owner files and budgets.**

| Layer | Files | Budget |
|---|---|---|
| Core, Rust | `loft-ir/src/{bundle,identity}.rs` (inputs, identity); `loft-core/src/{kernel,schedule,check}.rs` (fused ticks, order, `Scope::Trace`); `loft-gpu/src/lib.rs` (log ring, instance state, checkpoints, encode order) | ≤ 120 + 180 + 300 |
| Recorder, TS | `src/compute.ts` (keep latest, the input sum type) | ≤ 60 |
| Sub meta engine, TS | `src/motion/{body,move,index}.ts` | ≤ 400 |
| Library, WGSL | `src/motion/wgsl/{seams,support,pin,trunk,present}.wgsl` | ≤ 650 |
| Example engine | `src/motion/wgsl/roam.wgsl`, `src/motion/roam.ts` | ≤ 150 + 40 |
| Scene and checks | `examples/motion/crab_live.ts`, `rust/crates/loft-gpu/tests/it/motion_live.rs` | ≤ 120 TS, ≤ 250 Rust |

**Predicted, before building.** Motion rows come from these laws evaluated at f64 (F's script with greedy lifting,
the load and support gates, nonnegative shares and the exact integrator; scratch, not kept). An f32 threshold may move
a lift by a tick.

| # | Check | Predicted | Pass |
|---|---|---|---|
| 1 | Live against baked, same GPU and script, at 30, 60 and 120 Hz presentations and with a 6-tick stall | 0 of 481 states differ in any bit | 0 |
| 2 | Steps | 22; per foot 2, 3, 2, 3, 3, 3, 3, 3 | 20 to 24 |
| 3 | First lifts | 1.042 s (feet 0, 2, 5; foot 7 one tick later); all first lifts by 1.392 s | ±2 ticks |
| 4 | Reversal | The stick flips at 1.700 s, v crosses 0 at 1.900 s, the next lift is at 2.283 s | ±3 ticks |
| 5 | Clamp and lean | Peak acceleration 5.886 m/s² (friction); most lean 32.0° at 2.025 s | exact; ±0.5°, ±2 ticks |
| 6 | Support | Least share 0.060; p\* at least 1.62 m inside the hull; fewest feet down 4; the support gate binds 0 times on this wide body | all shares ≥ 0, margin > 0 |
| 7 | Reach | Margin at least 0.17 m; the leash never engages | margin > 0 |
| 8 | Plant on the drawn toe, after IK, in the deck frame | At most 0.03 mm (f32 rounding at 7 m); neighbours swinging together 0 | ≤ 0.1 mm; 0 |
| 9 | False plant: 2 cm added after IK to foot 3 | Fails at foot 3 only, 20.0 mm, over each of its stances, while the controller still says planted | exactly that |
| 10 | Landing | Last tick's travel at most 0.65 mm horizontally, 0 vertically; swings 0.120 to 0.334 s; lifts 0.250 to 0.985 | ±0.01 |
| 11 | At 4.0 s | z = −1.076 m, v = −0.086 m/s | ±0.02 |
| 12 | History | The same stick from 1.0 s after two different openings: feet differ by up to 0.26 m at 4 s, and the frame identities differ. The same history gives the same identity and RGBA | as stated |
| 13 | A law replaced: P's lower arch for urgent steps | One module's text changes. Lift ticks are identical, because decisions read only xz. Swing heights scale | as stated |
| 14 | GPU, motion span | One dispatch per frame whatever N and the ticks owed. 1 crab and 256 crabs: p95 below one quantum. 1,000 crabs: 0.03 to 0.09 ms | ≤ 0.065 ms; ≤ 0.2 ms |
| 15 | GPU, whole frame | Within one quantum of the same draws fed a recorded POSE table | ≤ 0.065 ms difference |
| 16 | Bake | 480 ticks in one dispatch, at most 3.6 ms | ≤ 5 ms |
| 17 | Modules and load | One crab and five crabs of different bodies build identical module hashes: 1 motion module and 2 pipelines, at most 8 modules in all. Frame times differ by at most a quantum. Motion's cold setup is 0.1 s | identical; ≤ 0.45 s |
| 18 | Bytes | STATE 672 B, POSE 928 B, kept take 323 KB | exact |
| 19 | Tokens | The crab file, 235 | ≤ 250 |
| 20 | Layers | `rg -i -w 'foot\|gait\|creature\|trunk' rust/crates` is empty | empty |
| 21 | The picture | Beside the look round's panels (a) and (c), at game distance, by the owner: it reads as weight at least as well as (c), the reversal reads as braking on planted feet, and no foot moonwalks | the owner |

A miss fixes the theory first. If the picture loses to (c), stop and derive the laws again. If Rust cannot yet draw
the declared pass, checks 1 to 20 score headless and only check 21 waits for the draws lane.

### Phases 2 to 5

| Phase | Adds | Owner files and budget per layer | Rust | Predicted checks |
|---|---|---|---|---|
| **2 The heron** | `head` kind (§4.7), `turn`, plans, `hop`, yaw clamped by support, decks (pins in the deck frame), A's four-pad test | `src/motion/wgsl/{head,plan,neck,deck}.wgsl` +700; `roam` +60; `src/motion/kinds.ts` +100; core 0 | None: tests that phase 1's core suffices | Hop: stance 0.770 s, crouch 0.590 m at 0.513 s, push 2.02 g at 0.583 s, flight residual 0. Fish at 1 rad/s: neck 1.58 rad, gaze error 0, held to 1.73 rad/s; beyond, or with no foot down, the gaze gives way and says so. Neck error ≤ 0.2 mm; head pitch ≤ 15°, 0° after take-off; head jerk ≤ 430 m/s³ (round 4: 428); 0 rear-crossing flips. Four pads: [0.25, 0.50], least-norm 0.375. A 0.5 m/s deck carries a held pad 0.50 ± 0.001 m |
| **3 `act`, the boss, the lineup** | Scores, beats, authority, `shock`, `shaped`; `hand` and `loose`; hits as events; five instances in one pass (Keel step 8); the directed rail hop, its free channels solved near keys by the existing Rust `solve` after A's elimination; the lineup on POSE with the render lane's per-kind draws | Library +600 WGSL; `act` +200; TS +150; core +80 in `loft-gpu` | Keel's contract: inputs in, pixels out | The claw arrives at 0.700 s on the same tick with and without a hit at 0.5 s; wind-up 0.69 m at 0.37 s. Δv < R/T_c moves no foot. Antennae at ζ = 0.45 overshoot 20.5%. Two hits 15 ticks apart apply once each; replays bit-identical at 30, 60, 120 Hz. Hitting one of five bosses leaves four POSE buffers bit-identical. Lineup: ≤ 25 pipelines (from 5,538 modules), p50 ≤ 4.3 ms (from 97.2), load ≤ 2 s (from 23.1), judged by eye against the TS lineup |
| **4 Interop** | `clip()` (glTF, BVH, arrays); live `"pose"` input; asynchronous POSE readback; `loft export --gltf` | `src/motion/clip.ts` +500 TS; library +150; core +40 (named-table readback) | Readback never in Keel's frame loop | A captured walk on `human` and on the heron keeps every labelled stance ≤ 0.1 mm on the drawn foot, beside the clip's own slide. Exported keys match POSE within 0.1 mm at 61 samples, ≤ 1 mm between, or split. Disagreeing contact labels listed |
| **5 Any body, crowds, retirement** | Spore's grid, 12 bodies × 6 motions; 1,000 mixed bodies; lowerings kept only if they win (workgroup per creature, closed-form springs, P's span compression, A's dependency cut); F's shooting solve as an experiment; `gait`, `footing`, `replants`, stances, `lookAt`, `lively.ts` weight curves and `pendulum` drives retired by codemod | Library +800; core +100 (emit); −1,500 TS | Lowerings are Rust emit choices | Every truth hold held on every cell, the picture rate reported per cell; 1,000 mixed ≤ 0.2 ms; the workgroup lowering at least halves a hero's span; shooting ≤ 0.3 s. Walking bipeds stay a conjecture until the grid scores them |

Over all phases the core gains about 900 Rust lines, the front door about 1,150 TS, the library about 2,900 WGSL and
the example engines about 450. Lines live in the library and the engines, as `PHILOSOPHY.md` §0 asks.

## 11. Standing of the numbers, and what would change the design

**Standing of the numbers:**

- **Measured** in the repo: the 6.5 µs step; 8.5 to 33 µs per dispatch; the 0.065 ms quantum; the look round's
  0.59 ms frame and 3.1 ms bake; round 4's 0.17 mm neck and 428 m/s³ head jerk; the 7.6 s near-keys solve; the Rust
  lineup and Titan costs (`agent_notes/2026-11-19_lean_rust_draws.md`) and the TS lineup's 4.26 ms.
- **Derived** here: the jump and slam constants, 1.58 and 1.73 rad/s, the four-pad interval, 19.8 µs per lineup
  pipeline, and the operation, lane and byte counts. **Evaluated**: phase 1's motion rows, from the f64 script.
  Nothing has run on a GPU. **Assumed**: the constants marked conjecture in §8. Tokens: `o200k_base` on §1's blocks.

**What would change the design:**

- **Live and baked differ** in bits on one GPU: pin the arithmetic before anything else.
- **The crab reads mechanical** beside panel (c): change the load cap and the lift law, not add parameters.
- **The lineup stays dispatch-bound** after motion is rows: the remaining kernels are draws, and the render lane's
  per-kind rule is the next lever.
- **Laws fall outside the `wgsl` subset** (structs with arrays, bounded loops): no twin verdicts; `loft check` needs a GPU.
- **Two phases of fixes go into one law** without the picture improving: stop and write its failure class.


================================================================
# MOTION HANDOFF
# file: docs/handoff/2026-11-19_motion_chief_scientist.md
# what it is: the motion goal, measured look rounds, and the nine questions we most want answered
================================================================

# Handoff: the motion meta engine (for the chief scientist, 19 Nov 2026)

You have seen our measurements and written the language draft (`agent_notes/dump_manual_nov_19th_alpha_v1.md`). We
have adopted it for render (`docs/proposals/2026-11-20_meta_engine_plan.md`). Motion is the part we most want your
mind on before we build it. This note is self-contained: the goal, what we measured, what we designed, where we are
unsure, and what we would like back.

## The goal

One motion system that covers both ends, per body part and per moment:

- **Authored performance.** Springy, appealing, Disney-principled motion (anticipation, squash and stretch,
  follow-through, overlap, arcs, timing) for film and choreographed boss attacks. The animator directs; the system
  adds weight without fighting the direction.
- **Physical, adaptive motion, Spore-like.** Any body plan, described once, moves well with no new code: legs, necks,
  tails, claws. Real-time player control and sudden changes of direction, organic, with no prebaked cycles: feet plant
  and step from wherever they are, the body leans into acceleration, balance is kept or recovered, contact holds.
- **Constraints a consumer can state** (neck length and rotation range, gaze holding a point with the skull aligned,
  as birds do, within what the neck can reach; joint limits; reach; posture; timing) and **laws a consumer can write
  or replace** (springs, muscle-like actuation, gravity and momentum, contact and friction, balance, stepping, target
  tracking: turn the head toward something on the right while the body turns left, stop at the neck's limit, then turn
  the body).
- **Explicit state** (plans, momentum, centre of mass, contacts, controller phase, targets) and an explicit choice of
  where each part runs: closed form of time, baked table, or live per-frame GPU state.
- **Interop**: motion in from outside (clips, motion capture, learned generators such as NVIDIA MotionBricks that
  are trained per rig), retargeted to the body plan and corrected by the constraints; posed skeletons out. Loft does
  not run the models; it works well with people who do.
- **Meta engine stance**: the hard parts are exposed, rewritable code in the language. `roam` (adaptive locomotion)
  and `act` (performance) are example engines, not the system.

Priorities: the picture (does it read as alive and weighted), then GPU cost per creature, then tokens for the author,
then truth (a constraint reported as held is held, measured on the drawn result after IK).

## What we measured (look rounds and Phase 1, M4)

Comparison page with the videos: the owner's artifact "Gait vs Solve". Notes: `agent_notes/2026-11-19_motion.md` on
branch `feat/motion`, `agent_notes/2026-11-19_design/motion_step3_impl.md`.

Heron hop (2.4 s, a rail to clear, a block to land on):

| Take | Crouch | Max body pitch | Head | Read |
|---|---|---|---|---|
| (a) Hand keys | shallow | upright | rigid on body | smooth, a little too linear |
| (b) Solve, least jerk (cubic B-spline unknowns, holds on position, velocity, acceleration, flight acceleration = g) | 0.77 m, 4.8 g push | 67° nose-down | rigid on body, 0.39 m below the hips | far too springy, recoils, head jolts at the ground. Cause: no posture hold, and least jerk penalises change of acceleration, not magnitude |
| (c) Solve near the hand keys (`prefer near`, jerk 1e-7), pitch ≤ 20°, head level | 0.60 m (predicted 0.55 to 0.70) | 12° | 0.17° tilt | the hand look with a real arc and a clean landing |
| (d) (c) plus world-space gaze lock and neck IK (fixed-length neck curve between shoulder and head; level gaze, glance at the landing spot capped at 15° in the run-up only) | as (c) | −11.9° to +8.6° | head jerk 428 m/s³ (from 670) | owner liked the neck motion; the most heron-like |

Crab scramble (hard right then hard left):

| Take | Result |
|---|---|
| Gait paced by distance | moonwalks on the reversal |
| Weight-driven stepping: a recur over 8 feet at 120 steps/s; a foot steps when strain or capture-point error demands it, toward where the body is going, from where it is; lean ≈ atan(a/g); outside legs compress | 24 steps (predicted 20 to 36), lean 30.2° (predicted 30.2°), zero slide while planted, no neighbours swinging together; first lift after reversal 0.36 s (predicted ≤ 0.2 s, slow) |
| Swings by need (duration and lift from step length and urgency) | 0.12 to 0.35 s swings, 23 steps, less mechanical |
| Phase 1 on Rust (`roam`, all crabs fused into one dispatch) | 22 steps as predicted; live equals bake to the bit at 30, 60 and 120 Hz with a six-tick stall; planted-toe drift 0.0008 mm after IK; a planted 20 mm fault caught on the right toe; motion p95 0.070 / 0.101 / 0.086 ms for 1 / 256 / 1,000 crabs; bake 5.3 ms; 216 tokens of author code. Owner verdict on weight pending |

Also derived (Fable, step 2): anticipation from boundary conditions — stance 1.949 v/g, crouch 0.385 v²/g, push
2.02 g — gives the heron 0.59 m against the animator's 0.60 m.

## What we designed (to be judged, not inherited)

`docs/proposals/2026-11-19_motion_architecture.md` (consolidated from three independent designs):

- Core learns nothing about bodies. Three tables: BODY (plan: segments, joints, limits, masses, ends), STATE
  (per-instance live state with checkpoints), POSE (32-byte joint frames read by render).
- Four end kinds: `foot`, `head`, `hand`, `loose`. The **pin** is one constraint for foot, gaze and hand, with strain
  flowing to the root. Holds are measured on the drawn end after IK (the "fake foot" test).
- Physics as chosen acceleration clamped by planted pins; only the trunk is integrated.
- One step function, three homes: closed form, baked table, live.
- Directed build-time motion uses solve near hand keys (the version the owner liked).
- Fused ticks: all motion for all creatures in one dispatch. Your correction stands: bucket by (dependency wave,
  law kind) so every SIMD group runs one law.
- Your draft adds `law`, `solver` (colored Gauss-Seidel or Jacobi, shared memory per strand), `constraint -> Verdict`,
  `Attach[...]` vs `Target[...]`, `@checkpoint(every: 60)`, and invariant gates on solved motion.

## Where we are unsure (please push hardest here)

1. **One formulation or layers?** Is per-tick motion best posed as a small constrained optimisation per creature
   (targets from performance, laws as costs, pins and limits as constraints: a QP, an XPBD system, or an augmented
   Lagrangian like our build-time solve), or as ordered layers (performance → balance → stepping → IK → secondary)?
   What gives Disney appeal and physical plausibility without one fighting the other?
2. **Any body plan, Spore-like.** What is the minimal body description from which stepping, balance, gaze and reach
   work with no per-creature code? How do gait patterns (tripod, wave, gallop) emerge from load and support rather
   than being authored? How does capture-point stepping generalise to N legs, uneven terrain and moving support (a
   foot on a moving ferry deck)?
3. **Solvers on the GPU, deterministic.** Which solver family for contacts, pins and limits gives bit-identical live
   and baked runs per device, scales to 1,000 creatures under 0.5 ms, and converges well enough that verdicts are
   honest? Where must iterations be fixed, and how are iteration caps reported (Unknown)?
4. **Authored and physical together.** How do hand keys, imported clips or generated motion act as the prior
   (`near` targets) while laws add weight, follow-through and recovery? How is anticipation derived rather than
   keyed, and when should the author's timing win?
5. **Gaze and necks.** A formal model of gaze stabilisation with limited neck range and a body-turn handoff (track a
   fish to the right while the body turns left; stop at the limit; then turn). Head-bob as hold-then-thrust.
6. **Secondary motion.** Analytic springs on piecewise-polynomial drivers (closed form per segment, scrubbable) vs
   stepped springs; volume-preserving squash and stretch; overlap on chains (antennae, tails, feathers).
7. **Reaction.** A boss hit mid-attack: impulse response, recovery that changes when the first support slips, a second
   hit before recovery. What state must exist for that, and how is it checkpointed?
8. **Truth.** What a motion verdict should state (held with margin, missed by how much, unknown and why), measured
   where (on the drawn end after IK, in the frame of the surface it grips), and which invariants every creature should
   carry by default (head above hips, no foot slide while planted, joint limits).
9. **Interop.** The smallest retargeting contract (joint map, rest-pose alignment, scale, non-human maps) and how a
   learned generator plugs in as a prior without the engine running it.

## What we would like back

1. The motion part of the language: `body`, `law`, `solver`, `constraint`, state and checkpoints, written so a model
   author can describe a new creature and its behaviour in a few hundred tokens.
2. The formulation you would choose for §1 to §4, with the reasoning and what it gives up.
3. The three cases written in it, read cold: the crab scramble, the heron tracking a fish while hopping (neck limit,
   then the body turns), a boss reacting to a hit mid-attack and to a second hit before it recovers.
4. Their lowering: dispatches per frame (bucketed by wave and law), state bytes per creature, GPU cost for 1 and 1,000
   creatures, bake cost, and what is checkpointed.
5. The default invariant gates and verdicts.
6. The single choice you think matters most, given our numbers.

Be ambitious. Nothing here has to survive except the measured lessons.


================================================================
# FOLDING AND TRACING
# file: docs/folding_and_tracing.md
# what it is: old design note on domain maps, folds and tracers
================================================================

# Folding space, and the tracers that draw folded things (design note, 2026-11-10)

Status: design note of 2026-11-10. Since built: 3.1 and 3.2 (`draw.field`, `impostors`) and 3.3 (`draw.relief`);
DESIGN.md 7.7 and section 8 say what was built and what it costs, and override this note where they differ. Statements
about techniques are from the author's knowledge, not re-read today. Corrections from building 3.1 are marked below.

## 1. What a fold is

A fold is a map `F` from space to space that sends many points to one: a reflection (`abs`), a repeat (`mod`), a
rotation into one sector, a scaling into one shell. Draw a motif at `F(p)` and it appears at every point that folds
onto it. One motif, any number of copies, the same cost per query.

That only works for things you **ask**. It does nothing for things you **list**.

| The thing is | Folding means | Cost of copies |
|---|---|---|
| asked at a point: a distance field, a density, a height function, a texture, noise, a paint function | compose the map in front: `f(F(p))` | none |
| listed as elements: triangles, instances, splats, particles | enumerate the copies: an instance index mapped to a transform | per copy |

So "which 3D geometry works with folding" has a short answer: every representation that is evaluated by query, and
none that is enumerated. For enumerated geometry the same symmetry is still useful, but it becomes *instancing with a
generator*: `transform(i)` walks the group (lattice index to translation, sector index to rotation, spiral index `i`
to `rotate(i * phi) * scale(s^i)`). Branching recursion of depth N costs `b^N` instances.

In Loft's terms this is already the capability split: a `Field` folds by composition; a `Surface` folds its paint and
its displacement by composing in its parameter domain; copies of a `Surface` in space are `draw.points` with a
generator.

## 2. The folds

Each returns the folded point, a cell identity (for per-copy variation), and a **stretch factor**: how much the map
scales lengths locally. A distance evaluated after the map must be multiplied by that factor to be a distance in
world space again.

| Fold | Map | Stretch | Gives |
|---|---|---|---|
| mirror | `abs(x)`; across a plane: `p - 2 min(0, p.n) n` | 1 | bilateral symmetry; several planes iterated give kaleidoscopes and polyhedral symmetry |
| repeat | `mod(p + c/2, c) - c/2`; limited: `p - c clamp(round(p/c), -l, l)` | 1 | lattices; finite rows of teeth, ribs, oars |
| polar repeat | angle `mod 2 pi / n` | 1 | n-fold rings: petals, legs, a ring of teeth |
| screw | `(theta, z - pitch * theta)` then repeat | about 1 | helices, ropes, springs |
| scale and iterate | fold, scale by `s`, shift; repeat N times | `s^-N` | self-similar trees, coral, sponges (kaleidoscopic iterated function systems) |
| log-polar (2D), log-spherical (3D) | `(log r, direction)` then repeat in `log r` | `r` | recursion toward a point: nested shells, infinite zoom |
| spiral | log-polar sheared by the angle | `r` | logarithmic spirals: nautilus, horns, whirlpools |
| inversion | `p R^2 / |p|^2` | `|p|^2 / R^2` | Apollonian packings, bubbles within bubbles |
| twist, bend, noise warp | not length-preserving, not conformal | a bound only | organic irregularity; shortens tracer steps |

Three rules that decide whether a fold is safe:

- **Isometries are exact inside a cell, not across cells.** After a repeat, the nearest copy may be the neighbour's.
  Mirrors are always exact. Repeats are exact if the motif stays inside its cell; otherwise evaluate the neighbouring
  cells too (2 per axis is enough when the motif fits in a cell).
- **Conformal maps need one scalar.** Log-polar, inversion and uniform scale stretch equally in all directions, so
  multiplying by the stretch restores a true distance bound.
- **Everything else needs a safety factor.** Twist, bend and noise warps stretch differently in different directions;
  the tracer must shorten its steps by the largest stretch, or it will step through surfaces and leave holes.

Time folds the same way: `mod(time, period)` loops a motion, and a space-time object can be repeated along time.

## 3. The tracers

A tracer is a per-pixel evaluator for something you can ask. Four are worth having. In every case the triangles Loft
already draws provide the *bound* (where to start and stop), and the tracer supplies the detail. "Rasterise the
coordinate system, trace what is inside it."

### 3.1 Sphere tracing a distance field

For free-standing solids: recursive shells, fractal coral, a rock field.

- March along the view ray, stepping by the distance the field returns; stop when the distance is below the pixel's
  footprint at that range (`eps * t`), which is also automatic level of detail: far away, small folds fall below the
  footprint and the march ends early. Iteration depth can be cut the same way (stop folding when the feature is
  smaller than a pixel).
- Start and end the march at a proxy: a box, a sphere, or a strand's own hull drawn as ordinary triangles.
- Write colour and `frag_depth`, so traced solids intersect triangle bodies and the sea correctly.
- Normals from the field's gradient. Finite differences cost four more evaluations; a symbolic gradient costs about
  one (section 4). *Correction (2026-11-11):* not through a fold's loop. Differentiating `iterate` carries the
  derivatives through every level; on the coral it cost 27 field evaluations on the CPU and, on the GPU, about as much
  as the whole march (its registers slowed the rest of the shader), so the tracer uses the four differences.
- A static field is a table (2026-11-11): sampled once into a distance brick whose lookup is a lower bound, the march
  steps by it in free space and asks the formula only near the surface; or extracted once into a triangle mesh. Both
  are `draw.field` schedules (DESIGN.md 7.7), chosen by `auto` from the field's leaves and the formula's cost.
- Known costs: writing depth from the fragment stage disables early depth rejection, so proxies should be tight and
  drawn front to back; multisampling does not antialias a traced silhouette inside its proxy, so the edge must be
  faded from the footprint.

### 3.2 Analytic ray casting, with a grid walk

For many copies of a simple shape: rain, bubbles, pearls, studs, rigging lines.

- Ray against sphere, ellipsoid, capsule, rounded cone and box has a closed form: exact hit, exact normal, no steps.
- For a repeated lattice, fold the *ray* instead of the point: walk the cells the ray crosses (a 3D DDA), and in each
  cell intersect the cell's motif analytically. Cost is the number of cells crossed, not the number of copies.
- A mirrored domain is the same thing seen the other way: a ray reflecting off the fold planes is a straight ray
  through the tiled copies.
- This is also a second schedule for thin strands: a chain of rounded cones intersected per pixel does not alias the
  way a thin triangle tube does.

### 3.3 Shell tracing in a body's own coordinates

For relief that belongs to a creature: scales, spikes, barnacles, suckers, feathers.

- Draw the body's surface as now, but at `rho = 1 + h_max`: the top of a thin shell around it.
- In the fragment stage, express the view ray in body coordinates `(m, v, height)` using the surface's Jacobian at
  that pixel, and march down through the shell until `height <= H(F(m, v))`, where `H` is a height function and `F`
  a fold in `(m, v)`.
- The relief bends with the body because it is written in the body's coordinates, costs a bounded number of steps
  per pixel, and adds no triangles. Fragments that never hit are discarded, so spikes have real silhouettes.
- Error: the ray is treated as straight in body coordinates over the shell's thickness. That is second-order in
  curvature and acceptable for stylised work; it degrades where the body bends tightly, the same limit as folding of
  body coordinates.

### 3.4 Volumes through a fold

For repeated soft things: a field of clouds, endless rain streaks, foam cells.

- A closed-form integral (a Gaussian blob along a ray) does not survive a fold directly: the ray crosses many cells.
  Walk the cells as in 3.2 and add each cell's closed-form contribution, or march with a small fixed step count.
- Gradient and cellular noise are already this: a lattice fold with a per-cell hash.

## 4. What the expression layer needs

| Addition | Why |
|---|---|
| Symbolic differentiation on the DAG (`grad`, `jacobian`) | One mechanism, four uses: normals of traced fields, stretch factors of folds, the Jacobian of a chart (computed by finite differences on the CPU in the scene-graph proof, where it dominated the cost), and pixel footprints. The DAG is pure and hash-consed, so derivative rules per operator are enough and shared subexpressions stay shared. |
| A loop form (`iterate(n, state, step)`; built in `src/expr.ts`, a real loop in both backends) | Fold iteration was unrolled, which grows the shader with depth and fixes the count at compile time. A real loop keeps code small and lets depth depend on distance. |
| Screen-space derivative operators (fragment stage only) | Edge widths in pixels. Important trap: the derivative of a *folded* coordinate jumps at every seam and draws a line there. The footprint must come from the unfolded coordinate times the fold's stretch, which is what `grad` provides. |
| `Fold` as a value: `{ p, id, stretch }`, composable | So folds chain, and the stretch is carried instead of remembered. |
| 3D distance primitives and operators | sphere, box, capsule, rounded cone, torus; union, subtract, intersect and their smooth forms; round; shell; revolve and extrude of the existing 2D distance functions. |

## 5. New kinds of thing

```ts
solid({ sdf: (p) => Expr<"f32">, bounds, paint, steps })                // 3.1: sphere-traced inside a proxy
impostors({ cell, shape, bounds })                                         // 3.2: analytic per-cell ray cast
relief({ on: strand, height: (m, v) => Expr<"f32">, hMax, fold, steps })  // 3.3: shell-traced on a body
```

Each is a `Drawable`: it emits its proxy triangles and a fragment shader assembled from expressions. The host needs
nothing new; it already supports fragment-stage depth and arbitrary shaders.

## 6. A check that must exist before any of this renders

A fold that breaks the distance bound produces holes that look like a rendering bug. Catch it as arithmetic: sample
pairs of points on the CPU backend and assert `|d(a) - d(b)| <= |a - b|` (times the declared safety factor). The CPU
and GPU backends agree by test, so a field that passes on the CPU is safe to march on the GPU.

## 7. Not known

Where the marching cost goes, the published ways to cut it, and four schedules derived for Loft are in
[`tracing_sota.md`](tracing_sota.md) (2026-11-11), with pictures in [`marching/index.html`](marching/index.html).

- Step budgets and frame cost in a browser. (On this machine's GPU: DESIGN.md 7.7. Scattered loads from a storage
  buffer cost about a tenth of a coral evaluation each, so a brick only pays for an expensive formula.)
- How far shell tracing can be pushed before curvature error shows.
- Whether fragment-stage depth costs more than the triangles it saves at RTS camera distance, where most relief is
  below a pixel and should be dropped by level of detail anyway.


================================================================
# REFERENCES
# file: docs/handoff/2026-11-20_spec/03_REFERENCES.md
# what it is: papers, systems and APIs worth reading, grouped by design question (cited from memory)
================================================================

# References: papers, systems and APIs worth reading

Cited from memory; titles and years should be checked before quoting. Grouped by the design question each one bears on.

## Language, staging and compilation
- Ragan-Kelley et al., *Halide: a language and compiler for optimizing parallelism, locality, and recomputation in image processing pipelines* (PLDI 2013): algorithm separated from schedule; the model for `schedule` blocks.
- He, Fatahalian, Foley, *Slang: language mechanisms for extensible real-time shading systems* (SIGGRAPH 2018): modules, interfaces and generics for shaders without permutation explosion.
- Hu et al., *Taichi* (SIGGRAPH Asia 2019) and *DiffTaichi* (ICLR 2020): a staged kernel language with sparse data and differentiation.
- JAX (Bradbury et al.) and its tracing and staging model; `torch.fx`: what goes wrong when the host traces silently.
- Willsey et al., *egg: fast and extensible equality saturation* (POPL 2021) and *egglog* (PLDI 2023).
- Terra (DeVito et al., PLDI 2013): multi-stage programming with a low-level target.
- naga (wgpu's shader IR): building `naga::Module` directly and validating it.

## Implicit surfaces, tracing, bounds
- Hart, *Sphere tracing: a geometric method for the antialiased ray tracing of implicit surfaces* (1996): the Lipschitz condition.
- Duff, *Interval arithmetic and recursive subdivision for implicit functions and constructive solid geometry* (SIGGRAPH 1992).
- Keeter, *Massively parallel rendering of complex closed-form implicit surfaces* (SIGGRAPH 2020) and the `fidget` project: tape evaluation, interval pruning, the Tier 0 idea.
- Inigo Quilez's articles on distance functions, smooth minimum, domain repetition and its limits, normals of SDFs.
- Media Molecule, *Dreams* rendering talks (Evans, SIGGRAPH 2015 Advances): SDF sculpting at scale.
- Galin et al., *Segment tracing using local Lipschitz bounds* (Eurographics 2020).

## Visibility and GPU-driven rendering
- Burns and Hunt, *The visibility buffer: a cache-friendly approach to deferred shading* (JCGT 2013).
- Karis et al., *Nanite* (SIGGRAPH 2021 Advances): GPU-driven culling, clusters, software raster.
- Wihlidal, *Optimizing the graphics pipeline with compute* (GDC 2016).
- WebGPU spec: `drawIndirect`, `RenderBundle`, no `multi_draw_indirect` in core; wgpu native features for multi-draw.

## Materials and light transport
- Burley, *Physically-based shading at Disney* (SIGGRAPH 2012 course).
- Kulla and Conty, *Revisiting physically based shading at Imageworks* (SIGGRAPH 2017 course): multiple-scattering energy compensation.
- Heitz et al., *Real-time polygonal-light shading with linearly transformed cosines* (SIGGRAPH 2016).
- Belcour and Barla, *A practical extension to microfacet theory for the modeling of varying iridescence* (2017): thin film.
- Bitterli et al., *Spatiotemporal reservoir resampling (ReSTIR)* (SIGGRAPH 2020); Ouyang et al., *ReSTIR GI* (2021).
- Majercik et al., *Dynamic diffuse global illumination with ray-traced irradiance fields (DDGI)* (JCGT 2019).
- Pharr, Jakob, Humphreys, *Physically Based Rendering* (4th ed.); Mitsuba 3 (Jakob et al. 2022) for the reference renderer and differentiable rendering.
- Jensen et al., *A practical model for subsurface light transport* (2001); Christensen and Burley, *Approximate reflectance profiles* (2015).
- Novák et al., *Monte Carlo methods for volumetric light transport simulation* (2018 STAR): joint integration of overlapping media.

## Non-photorealistic rendering
- Gooch et al., *A non-photorealistic lighting model for automatic technical illustration* (1998).
- Bénard and Hertzmann, *Line drawings from 3D models: a tutorial* (2019).
- Arc System Works, *Guilty Gear Xrd's art style* (GDC 2015): authored normals, outlines, stepped light.

## Motion: procedural, physical, learned
- Hecker et al., *Real-time motion retargeting to highly varied user-created morphologies* (SIGGRAPH 2008): Spore.
- Witkin and Kass, *Spacetime constraints* (SIGGRAPH 1988).
- Pratt et al., *Capture point: a step toward humanoid push recovery* (2006).
- Yin, Loken, van de Panne, *SIMBICON* (SIGGRAPH 2007); Coros, Beaudoin, van de Panne, *Generalized biped walking control* (SIGGRAPH 2010); Coros et al., *Locomotion skills for simulated quadrupeds* (SIGGRAPH 2011).
- Müller et al., *Position based dynamics* (2007); Macklin, Müller, Chentanez, *XPBD* (2016); Macklin et al., *Small steps in physics simulation* (2019).
- Jakobsen, *Advanced character physics* (GDC 2001).
- Clavet, *Motion matching and the road to next-gen animation* (GDC 2016); Holden et al., *Learned motion matching* (SIGGRAPH 2020); Zhang et al., *Mode-adaptive neural networks for quadruped motion control* (SIGGRAPH 2018).
- Peng et al., *DeepMimic* (SIGGRAPH 2018), *AMP* (2021), *ASE* (2022).
- Rosen, *An indie approach to procedural animation* (GDC 2014, Overgrowth).
- NaturalMotion Euphoria (dynamic motion synthesis), Rain World's procedural creatures (Joar Jakobsson, talks).
- NVIDIA MotionBricks (SIGGRAPH 2026): in-betweening transformer plus VQ-VAE, trained per rig.
- Thomas and Johnston, *The Illusion of Life* (1981): the twelve principles.
- Bird head stabilisation: Frost, *The optokinetic basis of head-bobbing in the pigeon* (J. Exp. Biol. 1978); the head-bob hold and thrust phases.
- Featherstone, *Rigid Body Dynamics Algorithms* (2008): articulated bodies.


================================================================
# APPENDIX: LOFT CONTEXT
# file: docs/handoff/2026-11-20_spec/_loft_context.md
# what it is: the old model-facing API summary (what models were taught)
================================================================

# Loft

A scene imports from `../src/index.ts` and default-exports `scene({ ground, things, style, sun, cameras })`; `ground`
is `plane` (y = 0), `nothing` or `sea`. y is up. Write the maths of a function (a spine, a girth, paint, a field) as
WGSL in `wgsl` text and the structure (which things, where, cameras, checks) in TypeScript: both build one expression
graph that draws on the GPU and evaluates on the CPU, so checks read what is drawn. `time` is an argument, in seconds,
so any frame draws alone; `period` says when every motion repeats. A swept body's m runs 0 (root) to 1 (tip) and v 0
to 1 round it; a surface's u and v run 0 to 1; paint is a function of them. Give every scene two cameras: the whole
with room round it, and the focus. `strand`, `spiralTube`, `sheetOn`, `ropes` and `impostors` return things: put them
in `things`. `coil`, `fan`, `sheet`, `lathe`, `loft` and `extrude` return a bare surface: draw it,
`draw.surface(name, surface, { paint, grid })`.
Light is the sun (`sun: { toward, amount }` dims or colours it; amount 0 is a moonless night) and up to seven `lights`,
each a function of a world point returning `{ toward, amount, distance?, size? }`, such as `lamp(at, { colour, reach })`;
a lamp casts a shadow given shadow: true, a glow lights nothing by itself. Paint may return a struct: `base` (the colour)
and words of your own, of which the styles know gloss (0 to 1), metal and emits (linear light), and `pbr` also rough
and normal (a bump: `gradient` of a height); a style notes the words it does not read. Its `scatter` answers light
from a direction, its `shade` changes the finished colour.
Your own triangles draw with `draw.mesh`, a solid you can intersect with `draw.traced`. `pass` is another camera's
picture (a mirror) for an expression to read. `--samples 64 --shutter 0.05 --aperture 0.1` makes a still with motion
blur and depth of field. Meshes come out (the bake command writes glTF), none go in. `loft` is `bun src/cli/main.ts`.

```ts run
// examples/reed.ts
import { plane, scene, strand, toon, wgsl } from "../src/index.ts";

const reed = wgsl`
fn spine(m: f32) -> vec3f { return vec3(m * m * sin(time) * 0.6, m * 4.0, 0.0); }  // a point per m; the tip sways
fn girth(m: f32) -> f32 { return 0.25 - 0.2 * m; }`;

export default scene({
  ground: plane,
  things: [strand({ name: "reed", spine: reed.spine, girth: reed.girth, tips: ["open", "point"], paint: [0.85, 0.55, 0.2] })],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
  cameras: {
    whole: { kind: "perspective", eye: [6, 4, 8], target: [0, 2, 0], fovY: 0.6 },
    tip: { kind: "perspective", eye: [2.5, 4.5, 3], target: [0, 3.6, 0], fovY: 0.5 },
  },
});
```

```sh
loft render examples/reed.ts --out out/reed.webp --views whole,tip --size 640x400
loft frames examples/reed.ts --out out/reed_t.webp --times 0,0.8,1.6 --view whole --size 480x300
```

# WGSL text

`wgsl` returns a function per fn, taking expressions or numbers; it fills any slot a TypeScript function fills. A slot
may pass more arguments than a fn declares (paint passes u, v, at); a returned struct fills a record slot by field
name. In scope: the module's fns, structs and consts, `time`, the names below, and holes: `${x}` brings in a `param`,
an `input`, a solved curve, a TypeScript function (called in the text) or a number. The subset that checks, `solve`
and `gradient` read: f32, vec2f..vec4f, bool, mat2x2f, mat3x3f, constant arrays, structs; let, var, assignment,
swizzle writes, if/else, for over a counter with break and continue, early return. As in WGSL, round ties to even, %
truncates (mod floors) and 1 is an integer (write 1.0). A fn using anything else (integers, bit operations, textures,
derivatives, while) is raw: it renders, and checks refuse it by name and line. Lower down, a module draws
(`draw.shader`: vs and fs entries) or computes (`compute`), reading camera_ray(px), frame and holes.
`loft show examples/reed.ts --thing reed` prints the WGSL the GPU runs, your lines marked. Call form (`add`, `mul`,
`vec3`) is for what TypeScript does: mapping over parts, helpers.
Builtins (WGSL's meaning): abs, sign, floor, ceil, fract, trunc, round, sqrt, inverseSqrt, exp, exp2, log, log2, sin, cos, tan, asin, acos, atan, sinh, cosh, tanh, saturate, normalize, min, max, pow, atan2, step, mix, clamp, smoothstep, length, distance, dot, cross, fma, radians, degrees, reflect, faceForward, refract, select, all, any; vec2..vec4 and f32() construct.
Prelude (the library's functions, as in the API map): mod, pick, hash2, hash3, hash22, hash33, noise2, noise3, cellular2, cellular3, remap, remapClamped, polar, cartesian, rotate2, fresnel, smin, smax, luminance, hsv, sdCircle, sdBox, sdSegment, sdCapsule, sdRing, sdTriangleIsosceles, repeatAngle, stroke, coverage, sdSphere, sdBox3, sdRoundBox, sdCapsule3, sdRoundCone, sdTorus, sdCylinder, sdPlane, sdEllipsoid, union, subtract, intersect, smoothUnion, smoothSubtract, smoothIntersect, shell, elongate, revolve, extrude, fold2, fold3; structs PaintAt { world, normal, copy }, Ray { o, d, t0, t1 }; gradient(h, p); time.

# Craft (long form: docs/craft.md)

**Range in shape: change the construction, not the number.** A viewer recognises a shape's law, not its numbers: ten
spiral tubes with different curls are one shape. Make three to five far-apart candidates before refining any, small on
one sheet; write the weakest thing about each, pick one, push it further than feels safe. Each must change at least one
of: the spine (`through`, `wave`, `spiralLine`, a heading written in m with `tabulate`); the section (`aspect`,
`squareness`, `twist` as functions of m); the topology (tube `strand`, sheet `sheet` or `fan`, turned `lathe`, folded
`draw.field` with `fold3`, copies `impostors`); the construction (`extrude`, `loft`, `grow`, `tree`, relief
`draw.relief`). Otherwise it is a retune. Read a component's limits (`loft api spiralTube`: its law, what it cannot
draw, its relatives), then go one level down and write the rest. Examples and presets pull: take how a part attaches
from them, never a proportion; make first the candidate you would not have made.

**Set the target in words first;** there are no reference pictures, and without a target the first render becomes it.
In the file's top comment: the silhouette in one sentence, the three largest forms and their proportions, what makes
it unlike the nearest example, two or three inks and one accent, the beats of its motion, and predictions a render can
be scored against (a part's pixels from a named camera, what is in front of what, a beat's length).

**Range in style: compose parts, write a part.** Presets (`toon`, `woodblock`, `biolume`, `papercraft`) are four looks,
not range. Paint in body coordinates (`palette.countershade`, `palette.bands`, `palette.spot`, `tile`), choose which
parts carry ink, make sky and ground part of the picture. A `Style` is a plain value: spread one and replace `shade`;
a `post` pass changes the whole frame. Say what a thing is with a word in its paint or a `scatter` before you wrap the
style; a `shade` that ignores the style's colour drops out of every restyle. Light a night by what is in it: a dim sun, and a `lamp`
at the same point as the glow that shows it; check the subject is lit (`litBy`).

**Shapes that read.** Large forms first, judged in flat paint; silhouette before detail; proportion and carriage are
identity; detail in clusters, not spread evenly; contrast with what is behind decides whether a feature exists.

**Motion that reads.** Three beats, wind-up several times the act (`beats`); the wrong way first and past the mark
after (`anticipated`, `overshot`, `response`); loose parts lag (`dragged`, `swing`); nothing still or in step (`alive`,
`uneven`); a pose struck and held (`struck`); arcs and weight (`arc`, `squash`, `impact`, `footing`, `carriage`,
`gait`); temperament is timing (`dart`, `pendulum`); a warning before an action (`attack`, `telegraph`); then push it
(`exaggerated`). State only for what truly steps: a kick is a `jolt`, a trail is the thing at earlier times; `recur`
a grid, a flock, contact.

**Predict, then look.** Expressions run on the CPU (`compileJs`), so predict sizes and positions first and keep them as
exported `checks` (`minScreenSize`, `staysInView`). Look at your render small, from its camera, as a silhouette and
across the motion (frames, above); change the largest thing first.

# API map (`loft api` for all of it; `loft api <name>` for one entry whole)

`name(args)`; `{a, b?}` a spec's fields (`?` optional), `f(m)` a field or argument that is a function. `~` marks a module with one user so far: cut it down when your subject fights it.

## Core
operators: abs, and, atan2, clamp, cos, cross, distance, div, dot, eq, exp, f, floor, fract, ge, gt, le, length, lift, log, lt, mix, mod, ne, neg, normalize, not, or, pow, round, saturate, select, sign, sin, smoothstep, sqrt, step, sub, tan, vec2, vec3, vec4
expr: TAU; add; compileJs(roots, rows?); evaluate(e, env?); fetch(table, index, lane); input(given, ty, def); iterate(count, init, step(state, i), until?(state)); lib(fn, ...args); max; min; mul; namespaceSeed(); param(name, def, range?); pick(i, values); row(r, x); swizzle(a, sel); time; variable(name, ty)
text: wgsl(strings, ...holes)
graph_fn: graphFn(name, params, body(...a))
grad: gradient(e, p)
curve: keys(points, options?); ramp(colours, label?); through(m, count, point(j)); bspline(t, over, values); wave(m, spec); retarded(t, m, speed) [signal delay]; response(t, spec); pulse(t, t0, t1, ease?); ease(x, a, b)
map/types: NO_SKELETON
chart: carry(chart, f); compose(outer, inner); on(host, c); place(shape, chart); points(count, spec(i)); posed(count, spec(i)); posedList(given, specs); shifted(chart, by); size.even(k); size.axes(k); sized(chart, k); sizeLengths(z); turn.about(axis, angle); turn.axes(x, y); turn.{none}; inFrame(g, d); world
compute: compute(name, spec)
recur: recur(spec)
fold: fold2.repeatPolar(count); fold2.logPolar(); fold2.spiral({arms, cells}); fold3.repeatPolar(count, axis?); fold3.screw(pitch, count, axis?); fold3.helix(angle, rise, period, axis?); fold3.logSphere(); fold3.twist(rate, radius); fold3.bend(rate, radius)
warp: warp.paint(paint(u, v, at), w(p)); warp.field(field(p), w(p)); warp.surface(s, w(p)); warp.curve(c, w(u)); warp.points(p, count, w(i)); tile(w, motif(cell))
table: tabulate(given, fn(x), n, when)
map/surface: extrude(curve, by); framed(s, at); framedPose(s, at); lathe(profile(u), ends); loft(curves); mapNormal(map(u, v)); sheet(root, edge); surface(map(u, v), topology); thicken(s, half)
sweep: sweep(name, spine(m), section, transport)
transport: rotateAbout(v, axis, angle)
draw: draw.surface(name, surface, {paint(u, v, at), grid, sides, outline}); draw.glazed(name, surface, {colour, grid, phase, sceneDepth, detail?}); draw.curve(name, curve, {width(u), paint(u, v, at), segments}); draw.posed(name, poses, thing); draw.field(name, field(p), {bounds, paint(hit), steps, outline, schedule, safety?}); draw.relief(name, surface, {height(at), hMax, fold(p), paint(at), steps, grid, outline}); draw.mesh(name, mesh, {at?(row, i), paint, sides, outline, skeletons?}); draw.traced(name, {bound, hit(ray, copy), paint, outline, skeletons?}); draw.copies(name, {kind, copies, paint(hit), outline, minPixels, keepLevels, inkBack, carry?}); draw.density(name, medium, {bounds, sun?, sky?, forward?, shadowing?, detail?, haze?}); draw.points(name, count, speck(index), o?); draw.volume(name, density(p), {bounds, steps, albedo?(p), emission?(p), g?, shadow?}); draw.shader(name, module, o)
field: fieldPixel
draw/surface: spacing.even(count); spacing.ends(count, crowd?); spacing.from(count, start)
scene: scene({ground, things, style, compute?, sun, lights?, cameras?, outline?, period?})
ground: plane; nothing
compile: compile(scene)
context: drawSpec(d)
bind: emitFn(slots, name, args, out, roots, ret); emitShaderFn(slots, f)
trace: bound.box(min, max); bound.sphere(centre, radius); bound.capsule(a, b, radius)
march: march(field(p), {within, steps, safety?})
cast: castRay(ray)
impostors: impostors(name, {cell, shape(cell), bounds, paint(hit), cells, outline})
height: offset(patch, u, v, a); segmentDistance(patch, u, v, a, b); pathDistance(patch, u, v, path); profile(p, r); bump(patch, {at, radius, height, profile}); ridge(patch, path, {width, height, profile}); crease(patch, path, {width, depth, sharpness}); weight(patch, r, u, v); flatten(patch, h(u, v), region, level); inflate(patch, h(u, v), region, amount); level(h); sum(...hs); smoothMax(k, ...hs); clampHeight(h(u, v), lo, hi); scaled(h(u, v), by(u, v))
sculpt: sculptLayer(name, samples, size, {wrap})
displace: displace(s, height(u, v))
light: lamp(at, {colour, reach, size?, shadow?}); movable(dir, name?)
pass: pass({name, size})
noise: noise2(p)

## Shapes
strand: strand({name, spine(m), stations?, girth, section?, twist?, tips?, paint, detail?, up?, level?, bulge?})
geom/sea: sea({swell, deform?, reach?, foam?})
fan: fan({rays, sag, scallop, slack?}); rayOf(body, v?); catenary(f, k)
spiral: spiralTube({name, length, curl, taper, girth, out, toward, twist?, paint, detail?}); spiralLine(spec); whorl(u, turns) [coil turn]; ridgePhase(u, turns, perTurn); coil(spec); coilAt(law); coilPlan(spec); COIL_TOPOLOGY
symmetry: mirror(part, name); array(part, count, placement(i)); reflect(p)
whirlpool: whirlpool({at, radius, spin, depth, drift, throat?, arms?})
afloat: afloat(w, {at, heading, length, beam, lift?, wind?}); afloatFrame(w, spec)
rig: rigid(g)
medium: bank(b); blob({centre, radii, density, colour}); layer({height, thickness, density, colour}); medium(name, {puffs?, banks?}); unionBox(boxes)
glow: flare(name, at, o); glow(name, curve(u), o); glows(name, s, {width, halo, colour, core, brightness(i), veil}); strokes(name, count, segments, point(u, i), taper?(u, i))
crest: sheetOn({on, from, to, height(x), v?, paint, sag?, rays?, rake?, flutter?, name, rows?}); sheetOnEdges(spec)
limb: chain({root, target, lengths, bend, foot?}); polyline(m, points, knots); unit(v)
lines: ropes(given, ropes, {width, colour, segments?})

## Motion
gait: gait({feet, phase, course(s), stride, pace, duty, lift, minStep?}); course.line(origin, heading, facing?); course.arc(origin, heading, radius, facing?)
motion: lookAt({from, target, yaw, pitch}); breathe({rate, depth, phase?, t?}); beat(r, {rate, amplitude, lag, bias?, down?, t?}); strike(s, {anticipation, follow, timing?})
lively: looping(seconds, build()); alive(t, amount, seed?, hz?, period?); anticipated(u, back?); arc(a, b, u, lift_, up?); beats(since, b); dragged(t, along, lag); echo(f(t)); exaggerated(x, rest, by); impact(after, depth?, hz?, damping?); overshot(u, back?); slowIn(u, power?); slowInOut(u, power?); slowOut(u, power?); squash(along); stretchOf(speed, per?, most?); swing(after, amount, hz?, damping?); uneven(part, spread?)
~stance: carriage(stances); footing({ground, homes, t?, replant?(t), stamps?, walk}); headingIn(ground, v); replants({homes, loop, busy(t), idleBelow, every, step, reach, seed}); towardIn(ground, p); carriedFrame(ground, c)
~placing: pose({at, heading, pitch, heel}); poseFrame(p); placedBy(p, {standIn, host}); blendFrames(a, b, w); turnedAbout(g, axis, angle); hosted(p); STAND_IN
~bend: bent({root, along, length, lean?, sharp?}); bentTip(o)
~pendulum: pendulum({length, damping}); sway(amplitude, period, phase?); jolt(after, speed)
~restless: dart(t, {every, period, seed, uneven?, past?}); chosen(hold, seed); tremor(t, hz, seed?)
~expression: cast(rest, named); score(cast_, layersAt(t)); show(name, weight); only(name, channels, weight); added(by, weight?); during(clock, {from, to?, rise, fall?})
~struck: struck(clock, {from, snap, hold, release, recoil?}); holdsStill(score, poses, {period, within, rate?})
~solve: solve(spec); spline(n, from?); equal(a, b, within?)

## Actions
hit: hit.capsule({a, b, radius}); hit.disc({centre, radius}); hit.arc({centre, inner, outer, from, to}); hitSdf(h, p); hitPatch(h, u, v, pad); hitContour(h, p, spacing); hitInradius(h)
attack: attack(spec); animate(rest(m), attacks, build(spine)); part(name, body, {from?, to?, tip}); once(at); every(period, first); hold(o); ring(o); INSTANT; STILL
telegraph: telegraph(attack, o?); DANGER
smear: smear(strike, {over, samples, speed?, opacity?})
wake: wake(body, {over, samples?, spread?, colour?})
design: design(spec); parseVariantAsk(text)
~emote: emote({name, arm, accents, lead, hold, fall, snap, offbeat, shuffle, settle})
~routine: routine({beat, length, centre, places, manner, size, score})
~ring: ringWave({centre, radius, since(t), speed, height, length, reach, hold, drag})
~lineup: lineup({members, water, loop})

## Checks
check: clearance(body, target); landsInside(strike, o?); gripReaches(attack, fingers, {middle, gap, rate?}); releasesAt(attack, {speed, place}); throwLands(attack, {least, speed, place, rate?}); fliesClear(attack, targets, {size, margin?, rate?}); letsGoClean(attack, fingers, thing, {seconds, depth, rate?}); shoves(ringing, place, {least, speed, place}); frontFirst(ringing, places, o?); warned(attack); tolerance.units(units); tolerance.share(share); minScreenSize(feature, camera, pixels, o?); litBy(light, feature, {least, from?, to?, at?}); budget({triangles}); runChecks(scene, checks, o?); formatChecks(vs); expressionsRead(score, marks, o); spansOnScreen(what, mark, camera, pixels, {at, size?}); staysInView(points, blockers, o); limbsReach(limbs, {period, slack, fold, rate?})
ranged: always(range, condition, o?); eventually(range, condition, o?)

## Styles
style: palette.countershade(v, {back, belly}); palette.bands(m, v, under, {colour, count, width, from?, to?}); palette.spot(m, v, under, {at, radius, colour, aspect?}); palette.clamp(c); dorsalness(v)
style: STYLES.toon(); STYLES.pbr(); STYLES.{woodblock, biolume, papercraft, sketch, neon}; STYLE_NAMES; styleNamed(name); withStyle(scene, style); withLight(scene, name)
style/light: light.core(edge?); light.highlight(o?); light.cast(o?); light.waterline(o?); light.overlap(o?); light.bokashi(o?); light.hatch(o?); light.rim(o?); light.block(o); light.recipe(r?); light.{key, edge}; LIGHT.{flat, twoTone, threeTone, bokashi, hatched, contact, rim, cel, sunset}; lit(r, ink, ctx); litBodies(recipe, own, ink(ctx), after?(c, ctx)); printedBlock(p, colour, tone); pullOf(b); markOf(p, tone); blockInk(b, own); keyOf(k, ctx)
sky: skyOf(sky, ctx)
style/sea: swellLines(ctx, {perWave, width})
paint: paint.hotter(o?); paint.mottle(o?); paint.swelling(o?); paint.wandering(o?); warmed(w, c); mottled(m, c, ctx); weighted(l, c, ink, ctx); standsOut(hue, c)
post: depthOfField({focus, focalLength, fStop}); filmGrain({amount}?); paperGrain(o?); bloom(o?); reflection(o?)
shared: edgeOf(x, edge, soft, widest?); pixelsInside(ctx); rgb(c); wgslRgb(c); POST_COMMON
gloss: glossed(colour, ctx, {gloss, metal?, seen?})
pbr_parts: ggx(n_h, rough); smithGgx(n_l, n_v, rough); schlick(v_h, f0); lambert(albedo, n_l, _n_v, _v_h, _rough); skyLight({normal, view, sun}, rough, sky(dir, sun)); daylight(dir, sun); aces(c)
toon: toon(spec?)
woodblock: woodblock(spec?); print.bands(o?); print.keys(keys); print.overprint({ink, amount, value, black}); seaPalette.swell(); seaPalette.deep(o?); LOOKS.{referencePalette, engraving, engravingSunset}; REFERENCE_PAINT; WOODBLOCK_PALETTE.{black, indigo, pale, cream, vermilion}; WOODBLOCK_SEA
biolume: biolume()
papercraft: papercraft()
sketch: sketch()
neon: neon()
pbr: pbr(parts?)
noise: noise3(p); hash2(p); hash22(p); cellular2(p)
util: luminance(rgb)

## Library (import from src/lib/index.ts)
noise: hash3(p); hash33(p); fbm2(p, spec); fbm3(p, spec); cellular3(p)
util: remap(x, inLo, inHi, outLo, outHi); remapClamped(x, inLo, inHi, outLo, outHi); polar(p); cartesian(pr); rotate2(p, angle); fresnel(normal, view, power); smin(a, b, k); smax(a, b, k); hsv(h, s, v)
sdf2: sdCircle(p, r); sdBox(p, halfSize); sdSegment(p, a, b); sdCapsule(p, a, b, r); sdRing(p, r, halfWidth); sdTriangleIsosceles(p, halfWidth, height); repeatAngle(p, count); stroke(d, halfWidth); coverage(d, pixelWidth)
fold: compose(first(p), second(p))
sdf3: sdSphere(p, r); sdBox3(p, halfSize); sdRoundBox(p, halfSize, r); sdCapsule3(p, a, b, r); sdRoundCone(p, a, b, r1, r2); sdTorus(p, major, minor); sdCylinder(p, r, halfHeight); sdPlane(p, normal, offset); sdEllipsoid(p, radii); union(a, b); subtract(a, b); intersect(a, b); smoothUnion(a, b, k); smoothSubtract(a, b, k); smoothIntersect(a, b, k); round(d, r); shell(d, thickness); elongate(sdf(p), h); revolve(sdf2(p), offset); extrude(sdf2(p), halfHeight); fieldOf(fold(p), sdf(p)); nearest(folds, sdf(p)); lipschitz(field(p), {samples, bounds, seed, separation?})
grow: kifs({count, fold, scale, offset}) [folded fractal]; grow({count, motif(q, level), step(q, level)}); levels(lo, hi, x)
similar: ask(chain); gen.shift(by); gen.turn(plane, angle); gen.mirror(axis); gen.shrink(ratio); list(chain, parent); partDistance(part, q); placePart(part, fr); toWorld(fr, q); tree({part, step}); WORLD

Domain modules, left out of this map (weather, sea and throwing): `loft api <name>` prints each: lightning, rain, cloud, whitewater, foam, grab.

Types you write: F, P3, In, Drawable, Style, Check, Frame, Ray, Traced, Branch, Chart, VolumeOptions, StrandSpec, Folded, Target, Grab.

# Wrong without an error

A one-sided surface wound inside out shows its far wall or nothing (try `sides: "two"`); a camera aimed past the
subject shows sky; a subject far too small or large for its camera; a `period` that is not every motion's own jumps at the loop.
`minScreenSize` and `staysInView` catch two. Exit 6 is the environment, not the scene.

# More

`loft api` (everything); `loft api spiralTube` (one entry whole, with its limits and relatives); docs/craft.md;
docs/guide.md.



================================================================
# APPENDIX: SIGNATURES
# file: docs/handoff/2026-11-20_spec/_signatures.md
# what it is: every exported author-facing signature of the old library (coverage record)
================================================================

# Current Loft TS API: every exported signature, first line (generated 2026-11-20 from main)

Engine internals (cli, host, compile, bake, record, core, frame, text) are omitted; this is the author-facing library.

## src/artefact.ts
```ts
export const CURVE_N = 64;
export const DETAIL_SLOTS = 4096;
export const PRELUDE_WGSL = /* wgsl */ `
export const SCENE_DEPTH_WGSL = /* wgsl */ `
export const CLEARANCE_TILE = 8;
export const CLEARANCE_MAX = 49;
export const CLEARANCE_SCALE = 64;
export const SCENE_CLEARANCE_WGSL = /* wgsl */ `
export const POST_WGSL = /* wgsl */ `${SCENE_DEPTH_WGSL}
export type Camera =
export type InputSlot = { readonly name: string; readonly ty: NumTy; readonly slot: number; readonly def: readonly number[] };
export type ParamSlot = { readonly name: string; readonly slot: number; readonly def: number; readonly min: number; readonly max: number };
export type DynamicBuffer = { readonly name: string; readonly binding: number; readonly vec4s: number };
export type StaticBuffer = { readonly name: string; readonly binding: number; readonly data: Float32Array };
export const POSE_WORKGROUP = 64;
export type PoseStage = { readonly label: string; readonly wgsl: string; readonly invocations: number };
export type TableFillKind =
export type BakeRecord =
export class Baked
export type Recurrence =
export type ReplayClock = { readonly slot: number; readonly rate: number; readonly start: number; readonly steps: number; readonly every: number };
export function replayClocks(rs: readonly Recurrence[] | undefined): ReplayClock[]
export function replayStep(r: ReplayClock, t: number): { readonly n: number; readonly k: number }
export type TableInfo = { readonly name: string; readonly buffer: string; readonly offset: number; readonly vec4s: number; readonly lanes: number; readonly fill: TableFillKind };
export type CpuUpload = { readonly buffer: string; readonly offset: number; readonly data: Float32Array };
export type Detail =
export const HYSTERESIS = 1.25;
export type Draw =
export type DrawDepth =
export type TextureSize = { readonly kind: "view"; readonly scale: number } | { readonly kind: "fixed"; readonly width: number; readonly height: number };
export type ArtefactPass =
export const PASS_CAMERA_VEC4S = 12;
export function passBindingsWgsl(passes: readonly ArtefactPass[]): string
export type PostPass = { readonly label: string; readonly shader: string };
export type AuthorLines = { readonly from: number; readonly lines: number; readonly file: string; readonly line: number; readonly what: string };
export function authorLinesProblem(x: unknown): string | undefined
export function authorLineAt(spans: readonly AuthorLines[], n: number): { readonly file: string; readonly line: number; readonly what: string } | undefined
export type FrameState = { readonly time: number; readonly inputs: Float32Array; readonly params: Float32Array; readonly curves: Float32Array };
export type Artefact =
export const LIGHT_SAMPLE_INPUT = "loft_xi";
export type View =
export const CELL_PIXELS = 7;
export type FrameStats = { readonly triangles: number; readonly draws: number };
export interface Host
export interface Loaded
export function handPose(dynamic: readonly DynamicBuffer[], pose: (state: FrameState) => Readonly<Record<string, Float32Array>>, why: string): Pick<Artefact, "stages" | "tables" | "cpuPose" | "pose">
export type Loop = { readonly kind: "open" } | { readonly kind: "every"; readonly seconds: number };
export const OPEN: Loop = { kind: "open" };
export function loopTime(l: Loop, t: number): number
```

## src/authored.ts
```ts
export type AuthorText = { readonly text: string; readonly file: string; readonly line: number; readonly what: string };
export function authored(t: AuthorText): void
export function authorLines(code: string): AuthorLines[]
```

## src/check.ts
```ts
export type Verdict =
export function checkOutcome(v: Verdict): "pass" | "fail" | "undecided"
export class Probe
export interface Check
export type Measured = Body | Part;
export function footprintShare(chain: readonly Joint[], outside: (x: number, z: number) => boolean, step: number): { readonly share: number; readonly area: number }
export function clearance(body: Measured, target: Target): Expr<"f32">
export type CheckTime = In<"f32"> | Arming;
export function landsInside(strike: Strike, o: { readonly tolerance?: number; readonly step?: number } = {}): Check
export function litBy(light: Light, feature: Measured, o: { readonly least: number; readonly from?: number; readonly to?: number; readonly at?: CheckTime }): Check
export function minScreenSize(feature: Measured, camera: string, pixels: number, o: { readonly at?: CheckTime; readonly size?: readonly [number, number] } = {}): Check
export type Mark = { readonly from: In<"vec3">; readonly to: In<"vec3"> };
export function expressionsRead<K extends string, N extends string>(score: Score<K, N>, marks: Readonly<Record<string, Mark>>, o:
export function spansOnScreen(what: string, mark: Mark, camera: string, pixels: number, o: { readonly at: CheckTime; readonly size?: readonly [number, number] }): Check
export function staysInView(points: Readonly<Record<string, In<"vec3">>>, blockers: readonly { readonly name: string; readonly skeletons: readonly Skeleton[] }[], o:
export type Tolerance = { readonly kind: "units"; readonly units: number } | { readonly kind: "share"; readonly share: number };
export const tolerance =
export type Reaching = { readonly name: string; readonly limb: ChainSpec; readonly outward: In<"vec3"> };
export function limbsReach(limbs: readonly Reaching[], o: { readonly period: number; readonly slack: Tolerance; readonly fold: Tolerance; readonly rate?: number }): Check
export function gripReaches(attack: Attack<Motion, Hold>, fingers: readonly [Measured, Measured], o: { readonly middle: Tolerance; readonly gap: Tolerance; readonly rate?: number }): Check
export function warned(attack: Attack): Check
export function releasesAt(attack: Attack<Motion, Hold<Thrown>>, o: { readonly speed: Tolerance; readonly place: Tolerance }): Check
export function throwLands(attack: Attack<Motion, Hold<Thrown>>, o: { readonly least: number; readonly speed: Tolerance; readonly place: Tolerance; readonly rate?: number }): Check
export function fliesClear(attack: Attack<Motion, Hold<Thrown>>, targets: readonly Target[], o: { readonly size: number; readonly margin?: number; readonly rate?: number }): Check
export function letsGoClean(attack: Attack<Motion, Hold<Thrown>>, fingers: readonly Measured[], thing: Target, o: { readonly seconds: number; readonly depth: Tolerance; readonly rate?: number }): Check
export type Ringing = Attack<Motion, Effect, Ring> | { readonly name: string; readonly wave: RingWave; readonly first: In<"f32"> };
export function shoves(ringing: Ringing, place: In<"vec2">, o: { readonly least: number; readonly speed: Tolerance; readonly place: Tolerance }): Check
export function frontFirst(ringing: Ringing, places: readonly In<"vec2">[], o: { readonly rate?: number } = {}): Check
export function budget(o: { readonly triangles: number }): Check
export function runChecks(scene: Scene, checks: readonly Check[], o: { readonly inputs?: Inputs; readonly compiled?: Compiled } = {}): Verdict[]
export function formatChecks(vs: readonly ReturnType<Check["run"]>[]): string
```

## src/check_state.ts
```ts
export function stateChecks(probe: Probe, loop: Loop): ReturnType<Check["run"]>[]
```

## src/compute.ts
```ts
export type ComputeResource = Shape &
export type ComputePass =
export function resourceError(where: string, given: string, expected: string, fix: string): never
export function keptStates(s: Steps): number { return 1 + Math.ceil(s.count / s.keep); }
export function passLibs(code: EntryText): LibFn[]
export function compute<const W extends Readonly<Record<string, Shape>>>(name: string, spec: Spec<W>): { readonly [K in keyof W]: Handle<W[K]> }
export function orderCompute(given: readonly object[] | undefined): ComputePass[]
```

## src/core.ts
```ts
export type { Artefact } from "./artefact.ts";
export type { Camera } from "./artefact.ts";
```

## src/curve.ts
```ts
export type Curve<T extends "f32" | "vec2" | "vec3" | "vec4" = "vec4"> = (x: In<"f32">) => Expr<T>;
export type Key = readonly [number, number | readonly number[]];
export type Interp = keyof typeof INTERP;
export function keys(points: readonly Key[], options: { interp?: Interp; label?: string } = {}): Curve<"vec4">
export function ramp(colours: readonly (readonly [number, number, number])[], label = "ramp"): Curve<"vec4">
export function through(m: In<"f32">, points: readonly In<"vec3">[]): Expr<"vec3">;
export function through(m: In<"f32">, count: number, point: (j: Expr<"f32">) => In<"vec3">): Expr<"vec3">;
export function through(m: In<"f32">, points: readonly In<"vec3">[] | number, point?: (j: Expr<"f32">) => In<"vec3">): Expr<"vec3">
export function bspline(t: In<"f32">, over: readonly [number, number], values: readonly In<"f32">[]): Expr<"f32">
export type WaveSpec =
export function wave(m: In<"f32">, spec: WaveSpec): Expr<"vec3">
export function retarded(t: In<"f32">, m: In<"f32">, speed: In<"f32">): Expr<"f32">
export type ResponseSpec =
export function response(t: In<"f32">, spec: ResponseSpec): Expr<"f32">
export function pulse(t: In<"f32">, t0: In<"f32">, t1: In<"f32">, ease?: In<"f32">): Expr<"f32">
export const ease = (x: In<"f32">, a: In<"f32">, b: In<"f32">): Expr<"f32"> => smoothstep(a, b, x);
```

## src/design.ts
```ts
export type Overrides<N> = { readonly [K in keyof N]?: N[K] extends readonly unknown[] ? N[K] : N[K] extends object ? Overrides<N[K]> : N[K] };
export type Widen<T> = T extends number ? number : T extends readonly unknown[] ? { readonly [K in keyof T]: Widen<T[K]> } : T extends object ? { readonly [K in keyof T]: Widen<T[K]> } : T;
export type Option<N> = { readonly about: string; readonly numbers?: Overrides<N> };
export type SlotTable<N> = { readonly [slot: string]: { readonly base: Option<N>; readonly [option: string]: Option<N> } };
export type Choice<T> = { readonly [S in keyof T]: keyof T[S] & string };
export type Picked<T> = { readonly [S in keyof T]: T[S][keyof T[S]] };
export type VariantSpec<N, T> = { readonly about: string; readonly pick?: Partial<Choice<T>>; readonly numbers?: Overrides<N> };
export type Built = { readonly scene: Scene; readonly checks: readonly Check[] };
export type Member = { readonly things: readonly Drawable[]; readonly checks: readonly Check[] };
export type Staged<M extends Member = Member> = { readonly deforms: readonly SeaDeform[]; readonly sea: SeaSpec; readonly finish: (water: Sea) => M };
export type DesignSpec<N, T extends SlotTable<N>, B extends Built = Built, H = never, M extends Member = Member> =
export type VariantAsk = { readonly variant: string; readonly picks: readonly (readonly [slot: string, option: string])[] };
export const BASE_ASK: VariantAsk = { variant: "base", picks: [] };
export type Resolved<N = unknown> =
export type DesignErrorKind = "syntax" | "unknown-variant" | "unknown-slot" | "unknown-option" | "unknown-number" | "conflict";
export class DesignError extends Error
export function parseVariantAsk(text: string): VariantAsk
export class Design<N, T extends SlotTable<N>, B extends Built = Built, H = never, M extends Member = Member>
export function design<N, T extends SlotTable<N>, B extends Built = Built, H = never, M extends Member = Member>(spec: DesignSpec<N, T, B, H, M>): Design<N, T, B, H, M>
export type AnyDesign = Design<unknown, SlotTable<unknown>>;
export type Subject =
export function subjectOf(mod: Record<string, unknown>, path: string): Subject
```

## src/errors.ts
```ts
export class SceneError extends Error
export class LoadError extends Error
export class EnvironmentError extends Error
export function shown(x: unknown): string
export function siteOf(stack: string | undefined): string | undefined
export function sitesOf(stack: string | undefined): string[]
export class RawComputeError extends Error
```

## src/expr.ts
```ts
export type Ty = "f32" | "vec2" | "vec3" | "vec4" | "bool";
export type NumTy = Exclude<Ty, "bool">;
export type VecTy = Exclude<NumTy, "f32">;
export class Row
export type ParamSpec = { readonly name: string; readonly def: number; readonly min: number; readonly max: number };
export type InputSpec = { readonly name: string; readonly ty: NumTy; readonly def: readonly number[] };
export type Fn =
export type LibFn =
export type LibRead =
export type TableSource = ProgramTable | DataTable | RecurrenceTable;
export type ProgramTable = TableShape & { readonly program: TableProgram };
export type DataTable = TableShape & { readonly data: Float32Array };
export type RecurrenceTable = TableShape & { readonly recurrence: string };
export type MapRun = { readonly count: number; readonly lanes: (i: Expr<"f32">) => readonly Expr<"vec4">[] };
export type ScanStep = { readonly next: readonly Expr<NumTy>[]; readonly lanes: readonly Expr<"vec4">[] };
export type TableProgram =
export const mapOf = (entries: number, lanes: MapRun["lanes"]): TableProgram => ({ kind: "map", runs: [{ count: entries, lanes }] });
export const listOf = (entries: readonly (readonly Expr<"vec4">[])[]): TableProgram => ({ kind: "map", runs: entries.map((e) => ({ count: 1, lanes: () => e })) });
export type TableFill = (frame: TableFrame, out: Float32Array) => void;
export interface TableFrame
export type Loop =
export type Node =
export type Lit = readonly number[];
export type In<T extends Ty = Ty> = Expr<T> | (T extends "f32" ? number : T extends "bool" ? boolean : readonly number[]);
export type Num = Expr<NumTy> | number | readonly number[];
export class Expr<T extends Ty = Ty>
export function lift<T extends Ty>(x: In<T>): Expr<T>
export const f = (x: number): Expr<"f32"> => lift<"f32">(x);
export function variable<T extends Ty>(name: string, ty: T): Expr<T>
export function namespaced<T>(id: string, build: () => T): T
export const currentNamespace = (): string => namespace;
export function namespaceSeed(): number
export function scopedName(name: string): string
export function input<T extends NumTy>(given: string, ty: T, def: readonly number[]): Expr<T>
export const time: Expr<"f32"> = input("time", "f32", [0]);
export function param(name: string, def: number, range?: { min?: number; max?: number }): Expr<"f32">
export function row(r: Row, x: In<"f32">): Expr<"vec4">
export const rawCall = Symbol("raw call");
export type RawCall = { readonly lib: LibFn; readonly trailing: readonly (number | Expr)[] };
export function fetch(table: TableSource, index: In<"f32">, lane: number): Expr<"vec4">
export function iterate<const S extends readonly Expr<NumTy>[]>(count: In<"f32">, init: S, step: (state: S, i: Expr<"f32">) => S, until?: (state: S) => Expr<"bool">): S
export function rawComputeOf(fn: LibFn): string | undefined { return rawCompute.get(fn)?.resource; }
export function rawComputeExpression(fn: LibFn, resource: string, ...args: In[]): Expr<"vec4">
export function markRaw(fn: LibFn, reason: string): void { rawReasons.set(fn, reason); }
export function rawReasonOf(fn: LibFn): string | undefined { return rawReasons.get(fn); }
export function lib<T extends Ty>(fn: LibFn, ...args: In[]): Expr<T>
export const vec2 = (...p: Num[]): Expr<"vec2"> => vecOf("vec2", p) as Expr<"vec2">;
export const vec3 = (...p: Num[]): Expr<"vec3"> => vecOf("vec3", p) as Expr<"vec3">;
export const vec4 = (...p: Num[]): Expr<"vec4"> => vecOf("vec4", p) as Expr<"vec4">;
export const identitiesOn = (): boolean => identities > 0;
export function withIdentities<T>(f: () => T): T
export const add: Assoc = assoc("add", (a: In<NumTy>, b: In<NumTy>) =>
export const sub = binary("sub");
export const mul: Assoc = assoc("mul", (a: In<NumTy>, b: In<NumTy>) =>
export const div = binary("div");
export const min: Assoc = assoc("min", binary("min"));
export const max: Assoc = assoc("max", binary("max"));
export const pow = binary("pow");
export const mod = binary("mod");
export const atan2 = binary("atan2");
export const step = binary("step");
export const neg = unary("neg");
export const abs = unary("abs");
export const sign = unary("sign");
export const floor = unary("floor");
export const fract = unary("fract");
export const sqrt = unary("sqrt");
export const exp = unary("exp");
export const log = unary("log");
export const sin = unary("sin");
export const cos = unary("cos");
export const tan = unary("tan");
export const saturate = unary("saturate");
export const normalize = unary("normalize");
export const ceil = unary("ceil");
export const trunc = unary("trunc");
export const roundEven = unary("roundEven");
export const rem = binary("rem");
export const inverseSqrt = unary("inverseSqrt");
export const exp2 = unary("exp2");
export const log2 = unary("log2");
export const asin = unary("asin");
export const acos = unary("acos");
export const atan = unary("atan");
export const sinh = unary("sinh");
export const cosh = unary("cosh");
export const tanh = unary("tanh");
export function mix(a: number, b: number, t: In<NumTy>): Expr<"f32">;
export function mix<A extends NumTy, B extends NumTy>(a: Expr<A>, b: Expr<B>, t: In<NumTy>): Expr<Wider<A, B>>;
export function mix<A extends NumTy>(a: Expr<A>, b: number, t: In<NumTy>): Expr<A>;
export function mix<B extends NumTy>(a: number, b: Expr<B>, t: In<NumTy>): Expr<B>;
export function mix<A extends NumTy>(a: Expr<A>, b: Lit, t: In<NumTy>): Expr<A extends "f32" ? VecTy : A>;
export function mix<B extends NumTy>(a: Lit, b: Expr<B>, t: In<NumTy>): Expr<B extends "f32" ? VecTy : B>;
export function mix(a: Lit, b: Lit, t: In<NumTy>): Expr<VecTy>;
export function mix<A extends NumTy = "f32", B extends NumTy = "f32">(a: Expr<A> | number | Lit, b: Expr<B> | number | Lit, t: In<NumTy>): Expr<Wider<A, B>>;
export function mix(a: In<NumTy>, b: In<NumTy>, t: In<NumTy>): Expr
export function clamp(x: number, lo: number, hi: number): Expr<"f32">;
export function clamp<A extends NumTy>(x: Expr<A>, lo: In<NumTy>, hi: In<NumTy>): Expr<A>;
export function clamp(x: Lit, lo: In<NumTy>, hi: In<NumTy>): Expr<VecTy>;
export function clamp<A extends NumTy = "f32">(x: Expr<A> | number | Lit, lo: In<NumTy>, hi: In<NumTy>): Expr<A>;
export function clamp(x: In<NumTy>, lo: In<NumTy>, hi: In<NumTy>): Expr
export function smoothstep(lo: number, hi: number, x: number): Expr<"f32">;
export function smoothstep<A extends NumTy>(lo: In<NumTy>, hi: In<NumTy>, x: Expr<A>): Expr<A>;
export function smoothstep<A extends NumTy = "f32">(lo: In<NumTy>, hi: In<NumTy>, x: Expr<A> | number | Lit): Expr<A>;
export function smoothstep(lo: In<NumTy>, hi: In<NumTy>, x: In<NumTy>): Expr
export const length = (a: In<NumTy>): Expr<"f32"> => call("length", "f32", [a]) as Expr<"f32">;
export const distance = (a: In<NumTy>, b: In<NumTy>): Expr<"f32"> => call("distance", "f32", [a, b]) as Expr<"f32">;
export const dot = (a: In<NumTy>, b: In<NumTy>): Expr<"f32"> => call("dot", "f32", [a, b]) as Expr<"f32">;
export const cross = (a: In<"vec3">, b: In<"vec3">): Expr<"vec3"> => call("cross", "vec3", [a, b]) as Expr<"vec3">;
export const lt = compare("lt");
export const gt = compare("gt");
export const le = compare("le");
export const ge = compare("ge");
export const eq = compare("eq"), ne = compare("ne");
export const and = (a: In<"bool">, b: In<"bool">): Expr<"bool"> => call("and", "bool", [a, b]) as Expr<"bool">;
export const or = (a: In<"bool">, b: In<"bool">): Expr<"bool"> => call("or", "bool", [a, b]) as Expr<"bool">;
export const not = (a: In<"bool">): Expr<"bool"> => call("not", "bool", [a]) as Expr<"bool">;
export function select(cond: In<"bool">, yes: number, no: number): Expr<"f32">;
export function select<A extends NumTy, B extends NumTy>(cond: In<"bool">, yes: Expr<A>, no: Expr<B>): Expr<Wider<A, B>>;
export function select<A extends NumTy>(cond: In<"bool">, yes: Expr<A>, no: number): Expr<A>;
export function select<B extends NumTy>(cond: In<"bool">, yes: number, no: Expr<B>): Expr<B>;
export function select<A extends NumTy>(cond: In<"bool">, yes: Expr<A>, no: Lit): Expr<A extends "f32" ? VecTy : A>;
export function select<B extends NumTy>(cond: In<"bool">, yes: Lit, no: Expr<B>): Expr<B extends "f32" ? VecTy : B>;
export function select<A extends NumTy = "f32", B extends NumTy = "f32">(cond: In<"bool">, yes: Expr<A> | number | Lit, no: Expr<B> | number | Lit): Expr<Wider<A, B>>;
export function select(cond: In<"bool">, yes: In<NumTy>, no: In<NumTy>): Expr
export function pick<T extends NumTy>(i: In<"f32">, values: readonly Expr<T>[]): Expr<T>;
export function pick(i: In<"f32">, values: readonly number[]): Expr<"f32">;
export function pick(i: In<"f32">, values: readonly (Expr<NumTy> | number)[]): Expr
export function round<A extends NumTy = "f32">(x: Expr<A> | number | Lit): Expr<A> { return floor(add(x as Expr<A>, 0.5)) as Expr<A>; }
export function swizzle<T extends NumTy>(a: Expr<VecTy>, sel: string): Expr<T> { return get<T>(a, sel); }
export const TAU = Math.PI * 2;
export function children(e: Expr): readonly Expr[]
export function everything(roots: readonly Expr[]): Expr[]
export function topo(roots: readonly Expr[]): Expr[]
export function splitOn(roots: readonly Expr[], vars: readonly Expr[]): { inner: Expr[]; outer: Expr[] }
export function leaves(roots: readonly Expr[]): { vars: Map<string, Ty>; inputs: InputSpec[]; params: ParamSpec[]; rows: Row[]; tables: TableSource[]; libs: LibFn[] }
export function libsOf(roots: readonly Expr[]): LibFn[]
export type Bind =
export const WGSL_TY: Readonly<Record<Ty, string>> = { f32: "f32", vec2: "vec2<f32>", vec3: "vec3<f32>", vec4: "vec4<f32>", bool: "bool" };
export function noteStatements(note: (e: Expr) => string | undefined): void { statementNote = note; }
export function emitWgsl(roots: readonly Expr[], bind: Bind, prefix = "e", pre: ReadonlyMap<number, string> = new Map(), comment?: (e: Expr) => string | undefined): { lines: string[]; out: string[]; libs: LibFn[]; names:
export function structuralKey(roots: readonly Expr[]): string
export function structuralHash(roots: readonly Expr[]): string
export function recordStacks(): void
export function stackOf(e: Expr): string | undefined { return (stacks?.get(e) ?? stacks?.get(rewrittenFrom(e)))?.stack; }
export function rewrittenFrom(e: Expr): Expr
export function substitute(roots: readonly Expr[], from: Expr, to: Expr): Expr[]
export function replace(roots: readonly Expr[], by: ReadonlyMap<number, Expr>): Expr[]
export type V = number | number[];
export type RowSource =
export type Env =
export function compileJs(roots: readonly Expr[], rows: RowSource = OWN_ROWS): (env: Env) => (V | boolean)[]
export function fold(roots: readonly Expr[]): Expr[]
export function evaluate(e: Expr, env: Partial<Env> = {}): V | boolean
```

## src/grad.ts
```ts
export type DerivativeFailure =
export class DerivativeError extends Error
export type DerivativeRule = (args: readonly Expr[], dargs: readonly Expr[]) => Expr;
export function defineDerivative(fn: LibFn, rule: DerivativeRule): void
export function derivative<T extends NumTy>(e: Expr<T>, x: Expr<"f32">): Expr<T>
export function derivativeAlong<T extends NumTy, P extends NumTy>(e: Expr<T>, p: Expr<P>, dir: In<P>): Expr<T>
export function gradient(e: Expr<"f32">, p: Expr<"vec2">): Expr<"vec2">;
export function gradient(e: Expr<"f32">, p: Expr<"vec3">): Expr<"vec3">;
export function gradient(e: Expr<"f32">, p: Expr<"vec2" | "vec3">): Expr<"vec2" | "vec3">
export function jacobian<T extends VecTy>(fx: Expr<T>, p: Expr<NumTy>): Expr<T>[]
export function normalOf(surface: Expr<"vec3">, u: Expr<"f32">, v: Expr<"f32">): Expr<"vec3">
export function stretchOf(map: Expr<"vec2" | "vec3">, p: Expr<"vec2" | "vec3">): Expr<"f32">
export type ScreenRate = { readonly of: Expr<NumTy>; readonly dx: In<NumTy>; readonly dy: In<NumTy> };
export type Footprint<T extends NumTy> = { readonly dx: Expr<T>; readonly dy: Expr<T>; readonly width: Expr<T> };
export function footprint<T extends NumTy>(coord: Expr<T>, px: readonly ScreenRate[]): Footprint<T>
export function noise2Expr(p: E): E
export function noise3Expr(p: E): E
```

## src/identity.ts
```ts
export type Program =
export type FrameCamera =
export type Identity =
export function identityId(i: Identity): string
export function canonical(v: unknown): string
export function programOf(bundle: Bundle): Program
export function libraryOf(): string
export function buildOf(bundle: Bundle): Identity["build"]
export function deviceOf(device: GPUDevice): Identity["device"]
export function frameOf(m: Pick<Manifest, "params" | "inputs">, view: View, precision: FramePrecision, drawn: StillSpec): Identity["frame"]
```

## src/index.ts
```ts
export type { Posed } from "./motion.ts";
```

## src/interval.ts
```ts
export type Tri = "true" | "false" | "either";
export class Unbounded extends Error
export type Refusal =
export class RangeRefusal extends SceneError
export type I = readonly [number, number];
export function bounds(x: C): I
export type Box = ReadonlyMap<string, I | Track>;
export type Track = { readonly centre: number; readonly slopes: ReadonlyMap<string, number> };
export type Work = { tableFills?: Compiled["tableFills"]; nodes: number; codomain: number; checkTime?: () => void; charge?: () => void };
export class Evaluator
```

## src/light.ts
```ts
export type V3 = readonly [number, number, number];
export type Sun =
export function movable(dir: V3, name = "sun"): { readonly kind: "input"; readonly name: string; readonly dir: V3 }
export function sunDir(s: Sun): V3 | Expr<"vec3">
export function sunParts(s: Sun): readonly [In<"f32">, In<"f32">, In<"f32">]
export type Arrival =
export type Arrived = Arrival & { readonly unshadowed: In<"vec3">; readonly visibility: In<"f32"> };
export type LightSource = { readonly kind: "point"; readonly at: Expr<"vec3"> } | { readonly kind: "direction"; readonly toward: In<"vec3"> };
export type LampShadow = { readonly at: Expr<"vec3">; readonly reach: number; readonly size: number };
export type Receiver = { readonly kind: "surface"; readonly at: Expr<"vec3">; readonly normal: Expr<"vec3"> } | { readonly kind: "point"; readonly at: Expr<"vec3"> };
export type ShadowTest = (r: Receiver, lit: Arrival) => Expr<"f32">;
export type Light = ((p: Expr<"vec3">, xi?: Expr<"vec2">) => Arrival) &
export function lamp(at: In<"vec3">, o: { readonly colour: In<"vec3">; readonly reach: number; readonly size?: number; readonly shadow?: boolean }): Light
export type SunAmount = { readonly kind: "full" } | { readonly kind: "given"; readonly amount: In<"vec3"> };
export type Sampling = { readonly kind: "point" } | { readonly kind: "sampled" };
export type Lighting = { readonly sun: Sun; readonly amount: SunAmount; readonly lights: readonly Light[]; readonly sampling: Sampling; readonly shadows: ReadonlyMap<Light, ShadowTest> };
export type Combine = "sum" | "largest";
export type Which = "sun" | "local";
export type LightFold = <T extends NumTy>(combine: Combine, term: (lit: Arrived, which: Which) => Expr<T>) => Expr<T>;
export function sunArrival(l: Lighting): Arrived
export function foldAt(l: Lighting, r: Receiver): LightFold
export const sunOnly = (l: Lighting): Lighting => ({ ...l, lights: [] });
```

## src/lineup.ts
```ts
export type Standing<N, H, M extends Member> =
export type Stageable<N, H, M extends Member> =
export type Lineup<M extends Member> =
export type LineupLoop = { readonly kind: "open" } | { readonly kind: "every"; readonly seconds: number };
export function lineup<M extends Member>(o: { readonly members: readonly Standing<unknown, unknown, M>[]; readonly water: Omit<SeaSpec, "deform">; readonly loop: LineupLoop }): Lineup<M>
```

## src/mistakes.ts
```ts
export function refuseSurface(name: string, surface: unknown, grid: unknown, look: { readonly kind: string; readonly sides?: unknown; readonly outline?: unknown }, point: () => unknown, paint: (() => unknown) | undefined
export function refuseScene(spec: { readonly ground?: unknown; readonly things?: unknown; readonly sun?: unknown; readonly cameras?: unknown }): void
```

## src/precision.ts
```ts
export const shadingTables = new WeakSet<TableSource>();
```

## src/ranged.ts
```ts
export type Range = readonly [Expr<"f32">, In<"f32">, In<"f32">];
export type Decision =
export type Spent =
export interface Ranged extends Check
export type Leaf = { readonly key: string; readonly label: string; readonly lo: Expr<"f32">; readonly hi: Expr<"f32"> };
export type Quantified = { readonly kind: "always" | "eventually"; readonly leaves: readonly Leaf[]; readonly body: Body; readonly label: string };
export type Settings =
export function measured(what: string, at: Measure["at"]): Expr<"f32">
export function measuredIn(roots: readonly Expr[]): Measure[]
export function always(range: Range, condition: Condition, o: Settings = {}): Ranged
export function eventually(range: Range, condition: Condition, o: Settings = {}): Ranged
export type Pinned =
export function pinned(check: Ranged, at: ReadonlyMap<string, number>, probe: Probe): Pinned
export function heldBy(probe: Probe): ConstructorParameters<typeof Evaluator>[1]
```

## src/scene.ts
```ts
export type SceneSpec =
export const MOST_LIGHTS = 7;
export type OutlineMethod = "hull" | "screen";
export const DEFAULT_OUTLINE: OutlineMethod = "hull";
export type Scene =
export function scene(spec: SceneSpec): Scene
export function restyled(scene: Scene, style: Style): Scene
```

## src/style.ts
```ts
export type Rgb = readonly [number, number, number];
export type ShadeCtx =
export type WaterCtx =
export type Contact =
export type LayerCtx =
export type SkyCtx =
export type GroundCtx =
export type ShadowBlock =
export type BlockCast =
export type BlockOverlap =
export type VolumeCtx =
export type Style =
export function dorsalness(v: Expr<"f32">): Expr<"f32">
export const palette =
export type Said = { readonly channels: ReadonlyMap<string, Expr<NumTy>>; readonly scatter: ((ctx: ShadeCtx, toward: Expr<"vec3">) => Expr<"vec3">) | undefined };
export type Reads = { readonly body: boolean; readonly channels: ReadonlyMap<string, NumTy>; readonly scatter: boolean; readonly geometry: boolean };
export function styleReads(style: Style, kind: ShadeCtx["geometry"]["kind"] = "unoccluded"): Reads
export function readsBody(style: Style): boolean
export type ShadeFields = Omit<ShadeCtx, "lights" | "channel" | "sky" | "scatter" | "geometry">;
export function shadeContext(f: ShadeFields, lighting: Lighting, style: Style, about: About = NOTHING_SAID, record: Recorder = NO_RECORD): ShadeCtx
export function shadeVariables(lighting: Lighting, style: Style, about: About = NOTHING_SAID): ShadeCtx
```

## src/draw/cast.ts
```ts
export const CAST_LIB: LibFn = { name: "loft_cast", ret: "vec4", wgsl: "", js: "gpu-only" };
export function castRay(ray: Ray): Traced & { readonly colour: P3; readonly id: Expr<"f32">; readonly m: Expr<"f32"> }
export type ChainInfo = { readonly first: number; readonly joints: number; readonly thing: number; readonly copy: number };
export type CastPlan = { readonly libs: readonly LibFn[]; wgsl(cpu: (roots: readonly Expr[]) => CpuFn, rest: Parameters<CpuFn>[0]): string };
export function castPlan(ctx: CompileContext, used: readonly LibFn[]): CastPlan | undefined
export function chainSpheres(things: readonly Drawable[]): { readonly infos: readonly ChainInfo[]; readonly at: TableSource; readonly table: TableSource }
export type CastJoint = { readonly p: V3; readonly r: number };
export function castCpu(o: V3, d: V3, t0: number, t1: number, w0: number, w1: number, chains: readonly (readonly CastJoint[])[], ground: boolean): { readonly t: number; readonly gap: number; readonly chain: number; reado
```

## src/draw/copies.ts
```ts
export type CopiesHit = { readonly p: P3; readonly normal: P3; readonly level: F; readonly id: Expr<"vec3"> };
export type CopiesOptions =
export class Copies implements Drawable
export function drawCopies(name: string, o: CopiesOptions): Copies
```

## src/draw/field.ts
```ts
export const fieldPixel: F = variable("loft_field_px", "f32");
export type FieldSchedule = "trace" | "brick" | "mesh" | "auto";
export type FieldHit = { readonly p: P3; readonly normal: P3 };
export type FieldOptions =
export type FieldSafetyFailure =
export type FieldChecker = "draw.field" | "march";
export class FieldScheduleError extends SceneError
export class FieldSafetyError extends SceneError
export const LIPSCHITZ_LIMIT = 1.01;
export const BRICK_CELLS = 256;
export function measureField(field: Field, b: TraceBound): number
export function checkField(name: string, field: Field, b: TraceBound, by: FieldChecker = "draw.field"): number
export function evaluationCost(e: Expr<"f32">, centre: readonly [number, number, number]): number
export const BRICK_WORTH = 1000;
export class FieldDrawable implements Drawable
export function fieldMesh(name: string, field: Field, b: TraceBound): TriangleMesh
export function drawField(name: string, field: Field, o: FieldOptions): FieldDrawable
```

## src/draw/field_shadow.ts
```ts
export type FieldShadow =
export function fieldShadow(field: Field, b: TraceBound, px: Expr<"f32">): FieldShadow
export function fitBlob(pts: readonly { readonly p: V3; readonly w: number }[], minRadius: number): readonly { readonly p: V3; readonly r: number }[]
export function shadowSkeleton(s: FieldShadow): Skeleton
```

## src/draw/grid_detail.ts
```ts
export function gridLevels(nu: number, nv: number, topology: Topology): readonly (readonly [number, number])[]
export type BoundCopies = { readonly kind: "one" } | { readonly kind: "poses"; readonly poses: Poses };
export type GridAt = { readonly u: (s: F) => F; readonly v: (s: F) => F };
export function boundTable(name: string, surface: Surface, at: GridAt, copies: BoundCopies): TableSource
export const GRID_S = variable("loft_grid_s", "f32"), GRID_T = variable("loft_grid_t", "f32");
export function gridDetail(bound: TableSource, surface: Surface, at: GridAt, copies: BoundCopies, nu: number, nv: number): DetailSpec
export const REST_CENTRE = { loft_grid_s: 0.5, loft_grid_t: 0.5 };
export const restPoints: readonly { readonly loft_grid_s: number; readonly loft_grid_t: number }[] =
export function measureRest(samples: readonly (readonly number[])[], anchor: readonly number[]): { readonly lengths: readonly [number, number]; readonly radius: number }
```

## src/draw/ground.ts
```ts
export type Plane = Drawable & { readonly kind: "plane" };
export type Nothing = Drawable & { readonly kind: "nothing" };
export type Ground = Sea | Plane | Nothing;
export const BOTTOMLESS = -1e9;
export class GroundError extends SceneError
export function levelOf(ground: Ground): (xz: Expr<"vec2">, t?: In<"f32">) => F
export function surfaceOf(ground: Ground, who: string): (xz: Expr<"vec2">, t?: In<"f32">) => F
export function seaOf(ground: Ground, who: string): Sea
export const plane: Plane =
export const nothing: Nothing =
```

## src/draw/impostors.ts
```ts
export type Cell = { readonly id: Expr<"vec3">; readonly centre: P3; readonly hash: Expr<"vec3"> };
export type ImpostorShape =
export type ImpostorHit = { readonly p: P3; readonly normal: P3; readonly local: P3; readonly cell: Cell };
export type ImpostorOptions =
export function shapeWgsl(kind: ImpostorShape["kind"]): string
export const CLOSEST_WGSL = /* wgsl */ `
export class Impostors implements Drawable
export function impostors(name: string, o: ImpostorOptions): Impostors
```

## src/draw/index.ts
```ts
export type { DensityOptions };
export type { PointsOptions, Speck } from "../soft/specks.ts";
export type { VolumeOptions };
export type { ShaderOptions };
export interface Poseable
export type Bounds = { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] };
export type { FieldOptions } from "./field.ts";
export type { ReliefOptions } from "./relief.ts";
export type { CopiesHit, CopiesOptions } from "./copies.ts";
export type { MeshData, MeshDrawable, MeshOptions } from "./mesh.ts";
export type { Ray, Traced, TracedDrawable, TracedOptions } from "./traced.ts";
export type CurveOptions =
export const draw =
```

## src/draw/ink.ts
```ts
export const MARK_WGSL = /* wgsl */ `
export const INK_POST = /* wgsl */ `
```

## src/draw/light.ts
```ts
export const light: Drawable =
```

## src/draw/march.ts
```ts
export function march(field: Field, o: { readonly within: TraceBound; readonly steps: number; readonly safety?: number }): (ray: Ray) => Traced
```

## src/draw/mesh.ts
```ts
export type MeshData =
export type MeshOptions =
export type Normals =
export type Mesh =
export type MeshPaint = (row: readonly F[], at: PaintAt) => P3 | Painted;
export function classifyMesh(name: string, data: MeshData, at: MeshOptions["at"]): { readonly mesh: Mesh; readonly motion: Motion }
export function fromNets(m: TriangleMesh): Mesh
export type MeshEmit = { readonly key: string; readonly show: string; readonly gate: string };
export function emitMesh(ctx: CompileContext, name: string, mesh: Mesh, motion: Motion, copies: Copies, o: { readonly paint: MeshPaint; readonly sides: "one" | "two"; readonly outline: "hull" | "none" }, e: MeshEmit): Em
export class MeshDrawable implements Drawable
export function drawMesh(name: string, mesh: MeshData, o: MeshOptions): MeshDrawable
export function meshPoint(mesh: Mesh, at: MeshOptions["at"], copies: Poses | undefined): { readonly world: P3; readonly normal: P3; readonly rowVars: (i: number) => Record<string, number[]> }
export const MESH_PAINT_VARS = { row: (width: number) => paintRowOf(width), at: (): PaintAt => vars().AT };
```

## src/draw/painted.ts
```ts
export type Answer = { readonly kind: "styled" } | { readonly kind: "wrapped"; readonly wrap: (ctx: ShadeCtx, styled: P3) => P3 };
export type Classified = { readonly base: P3; readonly said: Said; readonly answer: Answer };
export function classify(thing: string, x: P3 | Painted): Classified
export type Shading = { readonly kind: "shared" } | { readonly kind: "own"; readonly thing: string; readonly shader: Shader };
export type Notes = { readonly unread: readonly string[]; readonly ignoresStyle: boolean };
export function shadeCall(s: Shading, fn: "shade" | "ink" | "mark", shared: string, paintCall: string): string
export function tracedCall(s: Shading, fn: "shade" | "ink" | "mark", base: string, n: string, p: string, d: string, id: string, paintCall: string): string
export function withShading(paint: Shader, s: Shading): Shader
export function lookFns(ctx: CompileContext, thing: string, c: Classified, paintArgs: FnSpec["args"], geometry?: ShadeCtx["geometry"]): Shading
export function answered(a: Answer, ctx: ShadeCtx, styled: P3): P3
```

## src/draw/record.ts
```ts
export type RecordContext = Pick<CompileContext, "style" | "ground" | "sun" | "sunAt" | "lighting" | "things" | "outline"> &
export type Staged = { readonly phase: Phase; readonly draw: Draw };
export type Recorded = { readonly kind: "draws"; readonly draws: readonly Staged[] } | { readonly kind: "unported"; readonly what: string };
export const NONE: Recorded = { kind: "draws", draws: [] };
export const unported = (what: string): Recorded => ({ kind: "unported", what });
export const seaFootprint = raw.sea_footprint_at! as (p: Expr<"vec3">) => F;
export const screenDir = raw.screen_dir_at! as (uv: Expr<"vec2">) => Expr<"vec3">;
export const pixelOf = raw.pixel_of_point! as (p: Expr<"vec3">) => Expr<"vec2">;
export function cellsRole(name: string, counts: readonly [number, number], grid: Extract<Detail, { kind: "grid" }> | undefined, bound: (i: F) => Expr<"vec4">): Role
export const copyId = (id: number, i: F): F => fract(add(id, mul(i, 0.618034)));
export function litContext(ctx: RecordContext, at: { base: P3; normal: P3; world: P3; uv: Expr<"vec2">; anchor: P3; id: F }, about?: Parameters<typeof shadeContext>[3]): ShadeCtx
export function inkWidth(style: Style, p: P3): F
export function skyOf(ctx: RecordContext, dir: Expr<"vec3">): Expr<"vec3">
export const paintRole = (name: string, f: (p: P3, n: P3, uv: Expr<"vec2">, id: F) => Expr<"vec4">): Role =>
export type RayBound = { readonly kind: "box"; readonly box: { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] } } | { readonly kind: "copies"; readonly count: number; rea
export function rayDraw(ctx: RecordContext, name: string, bound: RayBound, hit: (ray: Ray, copy: F) => Traced, paint: (p: P3, n: P3, d: P3, copy: F) => P3 | Painted): Recorded
export function skyDraw(ctx: RecordContext): Staged
export function cameraOf(name: string, c: SceneCamera): Camera
export function sceneDecls(scene: Scene, artefact: Pick<Artefact, "draws">): { readonly scene: Decls; readonly unported: readonly string[] }
```

## src/draw/relief.ts
```ts
export type ReliefSite = { readonly u: F; readonly v: F; readonly cell: Folded<"vec2"> };
export type ReliefPoint = ReliefSite & { readonly h: F };
export type ReliefOptions =
export class ReliefDrawable implements Drawable
export function drawRelief(name: string, surface: Surface, o: ReliefOptions): ReliefDrawable
```

## src/draw/shader.ts
```ts
export type ShaderOptions = Pick<DrawSpec, "vertices"> & Partial<Pick<DrawSpec, "instances" | "phase" | "cull" | "depthWrite" | "blend">>;
export function drawShader(name: string, module: WgslModule, o: ShaderOptions): Drawable
```

## src/draw/shadows.ts
```ts
export const SHADOWS_TABLE = "shadows";
export const shadows: Drawable =
export type Chain = readonly { readonly p: Expr<"vec3">; readonly r: F }[];
export function jointList(chains: readonly Chain[]): TableSource
export function casters(things: readonly Drawable[]): { readonly chains: readonly Chain[]; readonly at: TableSource }
export function shadowTable(chains: readonly Chain[], seaHeight: (xz: Expr<"vec2">) => F, sun: readonly [In<"f32">, In<"f32">, In<"f32">], at: TableSource = jointList(chains)): TableSource
```

## src/draw/sky.ts
```ts
export const sky: Drawable =
```

## src/draw/surface.ts
```ts
export type Sampling = { readonly count: number; readonly at: (s: F) => F };
export type Grid = { readonly u: Sampling; readonly v: Sampling };
export const spacing =
export type Fragment =
export type Outline = "hull" | "border" | "none";
export type Look =
export type GlazedColour =
export type GlazedDetail = { readonly kind: "fixed" } | Extract<DetailSpec, { kind: "projected" }>;
export class SurfaceDrawable implements Drawable
export function posedJoint(j: { readonly p: P3; readonly r: F }, at: Posed): { readonly p: P3; readonly r: F }
export function withFragment(c: GlazedColour, g: (f: Fragment) => Fragment): GlazedColour
export type SurfaceOptions = { readonly paint: Paint; readonly grid: Grid; readonly sides: "one" | "two"; readonly outline: Outline };
export function drawSurface(name: string, surface: Surface, o: SurfaceOptions): SurfaceDrawable
export function drawGlazed(name: string, surface: Surface, o: { readonly colour: ((f: Fragment) => Expr<"vec4">) | GlazedColour; readonly grid: Grid; readonly phase: Extract<Phase, "water" | "on-water" | "transparent">; 
export function drawFlat(name: string, surface: Surface, o: { readonly paint: Paint; readonly grid: Grid }): SurfaceDrawable
export function nameId(name: string): number
```

## src/draw/trace.ts
```ts
export type TraceBound =
export const bound =
export function aabbOf(b: TraceBound): { readonly min: V3; readonly max: V3 }
export const PROXY_VERTICES = CUBE.length;
export function traceWgsl(b: TraceBound, show: string): string
export function tracePlumbingWgsl(): string
export function traceShadeWgsl(): string
export type TraceOutline = "silhouette" | "none";
export function outlineWgsl(o: TraceOutline): string
export const TRACE_DEPTH: { readonly kind: "entry"; readonly entry: "fs_depth" } = { kind: "entry", entry: "fs_depth" };
export const TRACE_DEPTH_OUT = "struct TraceDepthOut { @builtin(frag_depth) depth: f32 };";
```

## src/draw/traced.ts
```ts
export type Ray = { readonly o: P3; readonly d: P3; readonly t0: F; readonly t1: F; readonly width: (t: F) => F };
export type Traced = { readonly t: F; readonly gap: F; readonly normal: P3 };
export type TracedBound = TraceBound | { readonly kind: "copies"; readonly count: number; readonly box: (copy: F) => { readonly centre: P3; readonly half: P3 } };
export type TracedHit = { readonly p: P3; readonly normal: P3; readonly d: P3; readonly copy: F };
export type TracedPaint = (hit: TracedHit) => P3 | Painted;
export type TracedOptions =
export class TracedDrawable implements Drawable
export function drawTraced(name: string, o: TracedOptions): TracedDrawable
```

## src/draw/volume.ts
```ts
export type VolumeOptions =
export function drawVolume(name: string, density: (p: P3) => In<"f32">, o: VolumeOptions): VolumeDrawable
export class VolumeDrawable implements Drawable
export const TRANSMITTANCE: LibFn =
```

## src/engines/inspect.ts
```ts
export const inspect =
```

## src/engines/print.ts
```ts
export const flat = `fn shade(x:Point,wo:vec3f,ink:bool,px:vec2f)->vec4f
export const toon = `fn shade(x:Point,wo:vec3f,ink:bool,px:vec2f)->vec4f
export const print =
```

## src/engines/realtime.ts
```ts
export const realtime =
```

## src/engines/reference.ts
```ts
export const film = `fn present(c:vec3f)->vec4f
export function resolve(present: string, average: boolean): Source
export type Sampling = { samples?: number; depth?: number; present?: string };
export function sampling(o: Sampling)
export const jittered = `fn primary(px:vec2f,k:u32)->Ray
export const reference =
```

## src/geom/afloat.ts
```ts
export interface Waters
export type AfloatSpec =
export function afloatFrame(w: Waters, spec: AfloatSpec): Frame
export function afloat(w: Waters, spec: AfloatSpec): Chart
```

## src/geom/bend.ts
```ts
export type Bend = readonly [In<"f32">, In<"f32">];
export type BentSpec =
export type Bent =
export class BendError extends SceneError {}
export function bent(spec: BentSpec): Bent
export function bentTip(o:
```

## src/geom/crest.ts
```ts
export type SheetOnSpec =
export function sheetOn(spec: SheetOnSpec): SurfaceDrawable
export function sheetOnEdges(spec: Omit<SheetOnSpec, "paint" | "name" | "rows"> & { readonly name?: string }): { readonly root: Curve; readonly edge: Curve }
```

## src/geom/expression.ts
```ts
export type Expression<K extends string> = Readonly<Record<K, number>>;
export type Posed<K extends string> = Readonly<Record<K, F>>;
export type Cast<K extends string, N extends string> =
export class ExpressionError extends SceneError {}
export function cast<K extends string, N extends string>(rest: Expression<K>, named: Readonly<Record<N, Partial<Expression<K>>>>): Cast<K, N>
export type Layer<K extends string, N extends string> =
export const show = <N extends string>(name: N, weight: In<"f32">): Layer<never, N> => ({ kind: "show", show: name, weight });
export function only<K extends string, N extends string>(name: N, channels: readonly K[], weight: In<"f32">): Layer<K, N>;
export function only<K extends string>(to: Partial<Readonly<Record<K, number>>>, weight: In<"f32">): Layer<K, never>;
export function only<K extends string, N extends string>(a: N | Partial<Readonly<Record<K, number>>>, b: readonly K[] | In<"f32">, c?: In<"f32">): Layer<K, N>
export const added = <K extends string>(by: Partial<Readonly<Record<K, In<"f32">>>>, weight: In<"f32"> = 1): Layer<K, never> => ({ kind: "add", by, weight });
export function during(clock: In<"f32">, o: { readonly from: number; readonly to?: number; readonly rise: number; readonly fall?: number }): F
export type Score<K extends string, N extends string> =
export function score<K extends string, N extends string>(cast_: Cast<K, N>, layersAt: (t: F) => readonly Layer<NoInfer<K>, NoInfer<N>>[]): Score<K, N>
```

## src/geom/fan.ts
```ts
export type FanSpec =
export function catenary(f: F, k: number): F
export function fan(spec: FanSpec): Surface
export const rayOf = (body: Chart, v = 0.25): Curve => (m) => body.frame(vec3(m, v, 0)).o;
```

## src/geom/gait.ts
```ts
export type Course = (s: F) => Frame;
export type GaitSpec =
export type Pace =
export type Gait =
export function gait(spec: GaitSpec): Gait
export const course =
```

## src/geom/grab.ts
```ts
export type AxesSize = Extract<Size, { kind: "axes" }>;
export type GrabBeats = { readonly reach: number; readonly close: number; readonly hold: number; readonly release: number };
export type GrabSpec =
export type Grab =
export function grab(since: In<"f32">, spec: GrabSpec, b: GrabBeats): Grab
export type SetDown = { readonly kind: "setDown" };
export type Thrown =
export type Ending = SetDown | Thrown;
export const SET_DOWN: SetDown = { kind: "setDown" };
export type ThrownSpec = Omit<Thrown, "kind" | "land" | "up"> & { readonly land: In<"vec3">; /** Default +y. */ readonly up?: In<"vec3"> };
export function thrown(o: ThrownSpec): Thrown
export type Throw =
export function throwOf(windback: P3, e: Thrown): Throw
export type Leaving = { readonly palm: P3; readonly open: F; readonly home: F };
export function handTurn(e: Pick<Thrown, "flight" | "stroke">, turns: number): number
export const tumbleRate = (e: Pick<Thrown, "flight">, turns: number): number => (TAU * turns) / e.flight;
export function tumbled(g: Frame, axis: In<"vec3">, e: Pick<Thrown, "flight" | "stroke">, turns: number, at: { readonly whip: In<"f32">; readonly u: In<"f32"> }): Frame
export type Held =
export function heldThing(since: In<"f32">, seconds: number, from: P3, carry: (u: F) => P3, squeeze: number, pulses: number, end: Ending): Held
export type GripperSpec = Pick<GrabSpec, "rest" | "radius" | "reachLift" | "up" | "margin" | "restGap" | "windup">;
export function gripper(since: In<"f32">, spec: GripperSpec, b: GrabBeats, h: Held): Grab
export function fingerAngle(gap: In<"f32">, length: In<"f32">, hinge: In<"f32"> = 0): F
export function pincerAngle(gap: In<"f32">, length: In<"f32">): F
export type WrapSpec =
export function wrap(spec: WrapSpec): { readonly turns: F; spine(m: F): P3 }
```

## src/geom/limb.ts
```ts
export type ChainSpec =
export type Chain =
export const unit = (v: P3): P3 => div(v, max(length(v), 1e-6));
export function chain(spec: ChainSpec): Chain
export function polyline(m: F, points: readonly P3[], knots: readonly F[]): P3
```

## src/geom/lines.ts
```ts
export type Rope = { readonly a: P3; readonly b: P3; readonly sag: In<"f32"> };
export type RopesSpec =
export function ropes(given: string, ropes: readonly Rope[], spec: RopesSpec): Drawable
```

## src/geom/lively.ts
```ts
export type Beats = { readonly anticipate: number; readonly act: number; readonly settle: number };
export type Beat =
export function beats(since: In<"f32">, b: Beats): Beat
export function slowInOut(u: In<"f32">, power = 2): F
export const slowOut = (u: In<"f32">, power = 2): F => sub(1, pow(sub(1, clamp(u, 0, 1)), power));
export const slowIn = (u: In<"f32">, power = 2): F => pow(clamp(u, 0, 1), power);
export function anticipated(u: In<"f32">, back = 1.7): F
export function overshot(u: In<"f32">, back = 1.7): F
export function squash(along: In<"f32">): Size
export const stretchOf = (speed: In<"f32">, per = 0.15, most = 1.6): F => min(add(1, mul(per, speed)), most);
export function impact(after: In<"f32">, depth = 0.35, hz = 3, damping = 0.35): F
export function arc(a: In<"vec3">, b: In<"vec3">, u: In<"f32">, lift_: In<"f32">, up: In<"vec3"> = [0, 1, 0]): P3
export function swing(after: In<"f32">, amount: In<"f32">, hz = 1.4, damping = 0.25): F
export const dragged = (t: In<"f32">, along: In<"f32">, lag: In<"f32">): F => sub(t, mul(along, lag));
export const echo = (f: (t: F) => F, t: In<"f32">, delay: In<"f32">, gain: In<"f32">): F => mul(f(sub(t, delay)), gain);
export function looping<T>(seconds: number, build: () => T): T
export function alive(t: In<"f32">, amount: In<"f32">, seed: In<"f32"> = 0, hz = 0.23, period?: number): F
export const exaggerated = (x: In<"f32">, rest: In<"f32">, by: In<"f32">): F => add(rest, mul(sub(x, rest), by));
export const uneven = (part: In<"f32">, spread = 0.15): F => add(1, mul(sub(mul(hash3(vec3(part, 3.7, 9.2)), 2), 1), spread));
```

## src/geom/motion.ts
```ts
export type LookSpec =
export function lookAt(spec: LookSpec): { readonly yaw: F; readonly pitch: F; readonly dir: P3 }
export type BreathSpec = { readonly rate: In<"f32">; readonly depth: In<"f32">; readonly phase?: In<"f32">; readonly t?: In<"f32"> };
export function breathe(spec: BreathSpec): F
export type BeatSpec =
export function beat(r: In<"f32">, spec: BeatSpec): F
export type StrikeSpec =
export function strike(s: In<"f32">, spec: StrikeSpec): F
```

## src/geom/pendulum.ts
```ts
export type PendulumSpec =
export type Drive =
export const sway = (amplitude: In<"f32">, period: number, phase = 0): Drive => ({ kind: "sway", amplitude, period, phase });
export const jolt = (after: In<"f32">, speed: In<"f32">): Drive => ({ kind: "jolt", after, speed });
export class PendulumError extends SceneError {}
export type Pendulum =
export function pendulum(spec: PendulumSpec): Pendulum
```

## src/geom/placing.ts
```ts
export type Pose = { readonly at: P3; readonly heading: F; readonly pitch: F; readonly heel: F };
export function pose(o: { readonly at: In<"vec3">; readonly heading: In<"f32">; readonly pitch: In<"f32">; readonly heel: In<"f32"> }): Pose
export function poseFrame(p: Pose): Frame
export type StandIn = { readonly kind: "standIn" };
export type Hosted = { readonly kind: "host"; readonly pose: Pose };
export type Placer = StandIn | Hosted;
export const STAND_IN: StandIn = { kind: "standIn" };
export const hosted = (p: Pose): Hosted => ({ kind: "host", pose: p });
export function placedBy<T>(p: Placer, by: { readonly standIn: () => T; readonly host: (pose: Pose) => T }): T
export function blendFrames(a: Frame, b: Frame, w: In<"f32">): Frame
export function turnedAbout(g: Frame, axis: In<"vec3">, angle: In<"f32">): Frame
```

## src/geom/restless.ts
```ts
export type DartSpec =
export type Dart =
export class RestlessError extends SceneError {}
export function dart(t: In<"f32">, o: DartSpec): Dart
export const chosen = (hold: In<"f32">, seed: number): F => hash3(vec3(hold, seed, 5.3));
export const tremor = (t: In<"f32">, hz: number, seed = 0): F =>
```

## src/geom/rig.ts
```ts
export function rigid(g: Frame): Chart
```

## src/geom/ring.ts
```ts
export type RingSpec =
export type Ride =
export class RingWave implements SeaDeform
export function ringWave(spec: RingSpec): RingWave
```

## src/geom/sea.ts
```ts
export type SwellSpec =
export interface SeaDeform
export type SeaSpec =
export class Sea implements Chart, Drawable
export function sea(spec: SeaSpec): Sea
```

## src/geom/skin_distance.ts
```ts
export type V3 = readonly [number, number, number];
export type SkinDistance = { readonly lo: V3; readonly hi: V3; distance(q: V3): number };
export function skinDistance(skin: Surface): SkinDistance
export function skinBoxDistance(h: SkinDistance, q: V3): number
```

## src/geom/solve.ts
```ts
export type Family = { readonly kind: "spline"; readonly n: number; readonly from: F };
export function spline(n: number, from: In<"f32"> = 0): Family
export type HoldLine =
export function equal(a: In<NumTy>, b: In<NumTy>, within: number = TOLERANCE): Expr<"bool">
export function holdsOf(c: Hold, place: string): HoldLine[]
export type Prefer<K extends string> =
export type SolveSpec<K extends string> =
export type HoldReport = { readonly kind: HoldLine["kind"]; readonly said: string; readonly site: string; readonly when: string; readonly worst: number; readonly at: number };
export type SolveReport =
export type Solved<K extends string> =
export type SolveFailure =
export const TOLERANCE = 1e-5;
export class SolveError extends SceneError
export const JET: LibFn = { name: "loft_solve_jet", ret: "f32", wgsl: "fn loft_solve_jet(t: f32, a: f32, b: f32, c: f32, d: f32) -> f32 { return a; }", js: (_t: number, a: number) => a };
export function solve<K extends string>(spec: SolveSpec<K>): Solved<K>
```

## src/geom/spiral.ts
```ts
export type CoilLaw =
export type CoilSection =
export type CoilSpec = CoilLaw &
export function coil(spec: CoilSpec): Surface
export const COIL_TOPOLOGY = { u: ["cap", "free"], v: "periodic" } as const satisfies Topology;
export function coilAt(law: CoilLaw): (u: F, across: F, along: F) => P3
export function coilPlan(spec: CoilLaw & { readonly aspect?: number }):
export const whorl = (u: F, turns: number): F => mul(u, turns);
export const ridgePhase = (u: F, turns: number, perTurn: number): F => fract(mul(u, turns * perTurn));
export type SpiralTubeSpec =
export function spiralTube(spec: SpiralTubeSpec): Strand
export function spiralLine(spec: Pick<SpiralTubeSpec, "length" | "curl" | "taper" | "girth" | "out" | "toward" | "twist">): { readonly spine: (m: F) => P3; readonly girth: (m: F) => F; readonly up: P3 }
```

## src/geom/stance.ts
```ts
export type Carriage = { readonly surge: F; readonly rise: F; readonly sway: F; readonly roll: F; readonly pitch: F; readonly yaw: F };
export type Toward = { readonly x: F; readonly z: F };
export function towardIn(ground: Frame, p: P3): Toward
export function headingIn(ground: Frame, v: P3): Toward
export type Stance =
export function carriage(stances: readonly Stance[]): Carriage
export function carriedFrame(ground: Frame, c: Carriage): Frame
export type Replant = { readonly foot: number; readonly out: number; readonly back: number; readonly to: readonly [number, number] };
export type ReplantSpec =
export type StepTiming = { readonly lead: number; readonly seconds: number; readonly settle: number };
export function replants(spec: ReplantSpec): readonly Replant[]
export type Stamp = { readonly foot: number; readonly since: F; readonly lead: number; readonly height: number };
export type FootingSpec =
export type Footing =
export function footing(spec: FootingSpec): Footing
```

## src/geom/strand.ts
```ts
export type Rgb = readonly [number, number, number];
export type TipSpec = "round" | "point" | "open" | { readonly round: number } | { readonly point: number };
export type PaintSpec = Rgb | Paint;
export type StrandSpec =
export class Strand extends Body implements Drawable
export function strand(spec: StrandSpec): Strand
```

## src/geom/struck.ts
```ts
export class StruckError extends SceneError {}
export type StruckSpec =
export type Struck =
export function struck(clock: In<"f32">, o: StruckSpec): Struck
export function holdsStill<K extends string, N extends string>(score: Score<K, N>, poses: Readonly<Partial<Record<N, Struck>>>, o: { readonly period: number; readonly within: Tolerance; readonly rate?: number }): Check
```

## src/geom/symmetry.ts
```ts
export const reflect = (p: P3): P3 => vec3(p.x, p.y, neg(p.z));
export function mirror(part: Strand, name: string): Strand;
export function mirror(part: SurfaceDrawable, name: string): SurfaceDrawable;
export function mirror(part: Strand | SurfaceDrawable, name: string): Strand | SurfaceDrawable
export function array(part: Strand, count: number, placement: (i: number) => Chart): Strand[];
export function array(part: SurfaceDrawable, count: number, placement: (i: number) => Chart): SurfaceDrawable[];
export function array(part: Strand | SurfaceDrawable, count: number, placement: (i: number) => Chart): (Strand | SurfaceDrawable)[]
```

## src/geom/whirlpool.ts
```ts
export type WhirlpoolSpec =
export class Whirlpool implements SeaDeform
export class PoolChart implements Chart
export function whirlpool(spec: WhirlpoolSpec): Whirlpool
```

## src/kinds/materials.ts
```ts
export const matte = kind(`
export const lamp = (base: readonly number[], emission: readonly number[]) => (
export const glow = (power: readonly number[]) => lamp([0, 0, 0], power);
export const metal = kind(`
```

## src/kinds/shapes.ts
```ts
export const washer = kind(`
export const sphere = (radius: number) => ({ ...sphereKind(radius), radius });
export const bead = kind(`
export const box = kind(`
export const plane = kind(`
export const capsule = kind(`
```

## src/lib/fold.ts
```ts
export type Vec = "vec2" | "vec3";
export type Folded<V extends Vec> =
export type Fold<V extends Vec> = (p: In<V>) => Folded<V>;
export function compose<V extends Vec>(first: Fold<V>, second: Fold<V>): Fold<V>
export const fold2 =
export const fold3 =
```

## src/lib/grow.ts
```ts
export function kifs(spec: { readonly count: In<"f32">; readonly fold: Fold<"vec3">; readonly scale: number; readonly offset: In<"vec3"> }): Fold<"vec3">
export type Grown =
export type GrowSpec =
export function grow(spec: GrowSpec): (p: In<"vec3">) => Grown
export function levels(lo: number, hi: number, x: In<"f32">): F
```

## src/lib/noise.ts
```ts
export const hash2 = (p: In<"vec2">): Expr<"f32"> => lib<"f32">(HASH2, p);
export const hash3 = (p: In<"vec3">): Expr<"f32"> => lib<"f32">(HASH3, p);
export const hash22 = (p: In<"vec2">): Expr<"vec2"> => lib<"vec2">(HASH22, p);
export const hash33 = (p: In<"vec3">): Expr<"vec3"> => lib<"vec3">(HASH33, p);
export const noise2 = (p: In<"vec2">): Expr<"f32"> => lib<"f32">(NOISE2, p);
export const noise3 = (p: In<"vec3">): Expr<"f32"> => lib<"f32">(NOISE3, p);
export type Fbm = { readonly octaves: number; readonly lacunarity: In<"f32">; readonly gain: In<"f32"> };
export function fbm2(p: In<"vec2">, spec: Fbm): Expr<"f32">
export function fbm3(p: In<"vec3">, spec: Fbm): Expr<"f32">
export const cellular2 = (p: In<"vec2">): Expr<"vec4"> => lib<"vec4">(CELLULAR2, p);
export const cellular3 = (p: In<"vec3">): Expr<"vec2"> => lib<"vec2">(CELLULAR3, p);
```

## src/lib/screen.ts
```ts
export const fwidth: (x: In<"f32">) => F = screen.pixel_rate!;
export const viewDir: (p: In<"vec3">) => P3 = screen.toward_point!;
export const eyePosition: () => P3 = screen.eye_point!;
export const pixelWorld: (p: In<"vec3">) => F = screen.pixel_size!;
```

## src/lib/sdf2.ts
```ts
export function sdCircle(p: In<"vec2">, r: In<"f32">): Expr<"f32">
export function sdBox(p: In<"vec2">, halfSize: In<"vec2">): Expr<"f32">
export function sdSegment(p: In<"vec2">, a: In<"vec2">, b: In<"vec2">): Expr<"f32">
export function sdCapsule(p: In<"vec2">, a: In<"vec2">, b: In<"vec2">, r: In<"f32">): Expr<"f32">
export function sdRing(p: In<"vec2">, r: In<"f32">, halfWidth: In<"f32">): Expr<"f32">
export function sdTriangleIsosceles(p: In<"vec2">, halfWidth: In<"f32">, height: In<"f32">): Expr<"f32">
export function repeatAngle(p: In<"vec2">, count: In<"f32">): Expr<"vec2">
export function stroke(d: In<"f32">, halfWidth: In<"f32">): Expr<"f32">
export function coverage(d: In<"f32">, pixelWidth: In<"f32">): Expr<"f32">
```

## src/lib/sdf3.ts
```ts
export type Field<V extends Vec> = (p: Expr<V>) => Expr<"f32">;
export type Sdf3 = Field<"vec3">;
export function sdSphere(p: In<"vec3">, r: In<"f32">): Expr<"f32">
export function sdBox3(p: In<"vec3">, halfSize: In<"vec3">): Expr<"f32">
export function sdRoundBox(p: In<"vec3">, halfSize: In<"vec3">, r: In<"f32">): Expr<"f32">
export function sdCapsule3(p: In<"vec3">, a: In<"vec3">, b: In<"vec3">, r: In<"f32">): Expr<"f32">
export function sdRoundCone(p: In<"vec3">, a: In<"vec3">, b: In<"vec3">, r1: In<"f32">, r2: In<"f32">): Expr<"f32">
export function sdTorus(p: In<"vec3">, major: In<"f32">, minor: In<"f32">): Expr<"f32">
export function sdCylinder(p: In<"vec3">, r: In<"f32">, halfHeight: In<"f32">): Expr<"f32">
export function sdPlane(p: In<"vec3">, normal: In<"vec3">, offset: In<"f32">): Expr<"f32">
export function sdEllipsoid(p: In<"vec3">, radii: In<"vec3">): Expr<"f32">
export const union = (a: In<"f32">, b: In<"f32">): Expr<"f32"> => min(a, b);
export const subtract = (a: In<"f32">, b: In<"f32">): Expr<"f32"> => max(a, neg(lift<"f32">(b)));
export const intersect = (a: In<"f32">, b: In<"f32">): Expr<"f32"> => max(a, b);
export const smoothUnion = (a: In<"f32">, b: In<"f32">, k: In<"f32">): Expr<"f32"> => smin(a, b, k);
export const smoothSubtract = (a: In<"f32">, b: In<"f32">, k: In<"f32">): Expr<"f32"> => smax(a, neg(lift<"f32">(b)), k);
export const smoothIntersect = (a: In<"f32">, b: In<"f32">, k: In<"f32">): Expr<"f32"> => smax(a, b, k);
export const round = (d: In<"f32">, r: In<"f32">): Expr<"f32"> => sub(d, r);
export const shell = (d: In<"f32">, thickness: In<"f32">): Expr<"f32"> => sub(abs(lift<"f32">(d)), mul(thickness, 0.5));
export function elongate(sdf: Sdf3, h: In<"vec3">): Sdf3
export function revolve(sdf2: Field<"vec2">, offset: In<"f32">): Sdf3
export function extrude(sdf2: Field<"vec2">, halfHeight: In<"f32">): Sdf3
export function fieldOf<V extends Vec>(fold: Fold<V>, sdf: Field<V>): (p: In<V>) => Expr<"f32">
export function nearest<V extends Vec>(folds: readonly Fold<V>[], sdf: Field<V>): (p: In<V>) => Expr<"f32">
export type LipschitzSpec =
export function lipschitz<V extends Vec>(field: Field<V>, spec: LipschitzSpec): number
```

## src/lib/similar.ts
```ts
export type V3 = readonly [number, number, number];
export type Axis = "x" | "y" | "z";
export type Plane = "xy" | "xz" | "yz";
export type Gen =
export const gen =
export function ask(chain: readonly Gen[]): (p: In<"vec3">) => Folded<"vec3">
export type Frame = { readonly o: V3; readonly m: readonly [V3, V3, V3]; readonly scale: number };
export const WORLD: Frame = { o: [0, 0, 0], m: [[1, 0, 0], [0, 1, 0], [0, 0, 1]], scale: 1 };
export function toWorld(fr: Frame, q: V3): V3
export type Copy = { readonly frame: Frame; readonly id: V3 };
export function list(chain: readonly Gen[], parent: Frame): Copy[]
export type Part =
export function partDistance(part: Part, q: P3): F
export type Placed = { readonly a: V3; readonly ra: number; readonly b: V3; readonly rb: number };
export function placePart(part: Part, fr: Frame): Placed
export type TreeSpec =
export type Asked3 = { readonly d: F; readonly level: F; readonly id: P3 };
export type Branch = Placed & { readonly level: number; readonly id: V3 };
export type Tree =
export function tree(spec: TreeSpec): Tree
```

## src/lib/util.ts
```ts
export function remap(x: In<"f32">, inLo: In<"f32">, inHi: In<"f32">, outLo: In<"f32">, outHi: In<"f32">): Expr<"f32">
export function remapClamped(x: In<"f32">, inLo: In<"f32">, inHi: In<"f32">, outLo: In<"f32">, outHi: In<"f32">): Expr<"f32">
export function polar(p: In<"vec2">): Expr<"vec2">
export function cartesian(pr: In<"vec2">): Expr<"vec2">
export function rotate2(p: In<"vec2">, angle: In<"f32">): Expr<"vec2">
export function fresnel(normal: In<"vec3">, view: In<"vec3">, power: In<"f32">): Expr<"f32">
export function smin(a: In<"f32">, b: In<"f32">, k: In<"f32">): Expr<"f32">
export function smax(a: In<"f32">, b: In<"f32">, k: In<"f32">): Expr<"f32">
export function luminance(rgb: In<"vec3">): Expr<"f32">
export function hsv(h: In<"f32">, s: In<"f32">, v: In<"f32">): Expr<"vec3">
```

## src/light/queries.ts
```ts
export const albedoFill = { text: ggx + fill, opens: [] };
export const defaultSky = `fn sky(d:vec3f)->vec3f
export function lighting(s: Scene, w: ReturnType<typeof world>, fillModule: number)
```

## src/map/brick.ts
```ts
export const BRICK = 8;
export type Variance =
export function varianceOf(e: Expr): Variance
export type BrickSpec =
export type BrickStats = { readonly evaluations: number; readonly bricks: number; readonly coarse: number; readonly bytes: number; readonly ms: number };
export type BrickCell = { readonly at: V3; readonly slot: number };
export type Brick =
export function bakeBrick(name: string, d: Expr<"f32">, pvar: string, spec: BrickSpec): Brick
```

## src/map/chart.ts
```ts
export const rotate = (g: Frame, d: P3): P3 => add(add(mul(g.x, d.x), mul(g.y, d.y)), mul(g.z, d.z));
export function carry(chart: Chart, f: Frame): Frame
export const world: Chart = { frame: (c) => ({ o: c, x: X, y: Y, z: Z }), at: () => world, atNear: () => world };
export function on(host: Chart, c: In<"vec3">): Chart
export function compose(outer: Chart, inner: Chart): Chart
export function sized(chart: Chart, k: In<"vec3">): Chart
export function shifted(chart: Chart, by: In<"vec3">): Chart
export function sizeLengths(z: Size): P3
export function place<T>(shape: Placeable<T>, chart: Chart): T
export type PointSpec = { readonly at: In<"vec3">; readonly scale: In<"f32"> };
export function points(count: number, spec: (i: F) => PointSpec): PoseSet
export type Turn =
export const turn =
export const size =
export type PoseSpec = { readonly at: In<"vec3">; readonly turn: Turn; readonly size: Size };
export function turned(t: Turn): { readonly x: P3; readonly y: P3; readonly z: P3 }
export function posed(count: number, spec: (i: F) => PoseSpec): PoseSet
export function posedList(given: string, specs: readonly PoseSpec[]): Poses
export function sizeBound(s: Size): F
```

## src/map/nets.ts
```ts
export type TriangleMesh =
export type CpuField = (x: number, y: number, z: number) => number;
export function surfaceNets(brick: Brick, field: CpuField): TriangleMesh
```

## src/map/recur.ts
```ts
export type Cells = number | readonly [number, number];
export type Index<C extends Cells> = C extends number ? In<"f32"> : In<"vec2">;
export type Lanes = readonly Expr<"vec4">[];
export type RecurSpec<C extends Cells> =
export type Recurrence<C extends Cells> =
export const KEPT_BYTES = 64 * 1024 * 1024;
export const MOST_REPLAY = 120;
export type Kept = { readonly kind: "every" } | { readonly kind: "checkpoints"; readonly every: number };
export type Shape = { readonly kind: "list"; readonly n: number } | { readonly kind: "grid"; readonly w: number; readonly h: number };
export type RecurState =
export type RecurPart = "kept" | "even" | "odd" | "prev";
export const recurTable = (t: TableSource): { readonly state: RecurState; readonly part: RecurPart } | undefined => recurOf.get(t);
export function recur<const C extends Cells>(spec: RecurSpec<C>): Recurrence<C>
```

## src/map/surface.ts
```ts
export function mapNormal(map: (u: F, v: F) => P3): (u: F, v: F) => P3
export function atRest(point: (u: F, v: F) => P3): (u: F, v: F) => P3
export function surface(map: (u: F, v: F) => P3, topology: Topology): Surface
export function framed(s: Surface, at: Placed): Surface
export function framedPose(s: Surface, at: Posed): Surface
export function sheet(root: Curve, edge: Curve): Surface
export function lathe(profile: (u: F) => In<"vec2">, ends: readonly [End, End]): Surface
export function loft(curves: readonly Curve[]): Surface
export function extrude(curve: Curve, by: In<"vec3">): Surface
export function thicken(s: Surface, half: In<"f32">): Surface
```

## src/map/sweep.ts
```ts
export type Tip =
export type Section =
export type Transport =
export type FrameSample = { readonly p: P3; readonly der: P3; readonly t: P3; readonly d: P3; readonly s: P3; readonly stretch: F };
export function stationSample(t: TableSource, m: F): FrameSample
export type Slices = { readonly whole: Map<Expr<"f32">, Body>; readonly prefix: Map<string, Body>; readonly skeleton: Map<string, Skeleton>; count: number };
export const newSlices = (): Slices => ({ whole: new Map(), prefix: new Map(), skeleton: new Map(), count: 0 });
export type Posted =
export class Body implements Surface, Chart, Placeable<Body>
export class StationTable implements ProgramTable
export class SkeletonTable implements ProgramTable
export function sweepArgs(given: string, spine: (m: F) => P3, section: Section, transport: Transport, kind: "sweep" | "strand" = "sweep"): ConstructorParameters<typeof Body>
export function sweep(name: string, spine: (m: F) => P3, section: Section, transport: Transport): Body
```

## src/map/table.ts
```ts
export type Schedule = "static" | "frame" | "static-shading";
export function tabulate<T extends NumTy>(given: string, fn: (x: F) => Expr<T>, n: number, when: Schedule): (x: In<"f32">) => Expr<T>
```

## src/map/transport.ts
```ts
export const STATION_VEC4S = 3;
export type V3 = [number, number, number];
export type Q4 = [number, number, number, number];
export const v3 =
export function quatFromColumns(a: V3, b: V3, c: V3): Q4
export function qrot(q: Q4 | readonly number[], v: V3): V3
export const SAMPLE_VEC4S = 3;
export const SAMPLE = { world: 0, local: 1, rest: 2 } as const;
export type StationScan =
export function quatOf(a: P3, b: P3, c: P3): Expr<"vec4">
export function stationRadii(S: TableSource, k: F, leastStretch: number): { readonly a: F; readonly b: F; readonly stretch: F }
export function stationScan(s: StationScan): TableProgram
export function rotateAbout(v: In<"vec3">, axis: In<"vec3">, angle: In<"f32">): P3
```

## src/map/types.ts
```ts
export type F = Expr<"f32">;
export type P3 = Expr<"vec3">;
export type Frame = { readonly o: P3; readonly x: P3; readonly y: P3; readonly z: P3 };
export type Placed = { readonly frame: Frame; readonly scale: F };
export type Size = { readonly kind: "even"; readonly k: F } | { readonly kind: "axes"; readonly k: P3 };
export type Posed = { readonly frame: Frame; readonly size: Size };
export interface Poses
export type Curve = CurveOf<"vec3">;
export type End = "free" | "pole" | "cap";
export type Topology =
export interface Skeleton
export const NO_SKELETON: Skeleton = { joints: 0, joint: (k) => { throw new Error(`NO_SKELETON has no joint ${k}`); } };
export interface Surface
export interface Chart
export type Field = (p: P3) => F;
export type Density = (p: P3) => F;
export type PaintAt = { readonly world: P3; readonly normal: P3; readonly copy: F };
export type Paint = (u: F, v: F, at: PaintAt) => P3 | Painted;
export type Painted =
export type Warp<V extends "vec2" | "vec3"> = Fold<V>;
export interface Placeable<T>
```

## src/map/warp.ts
```ts
export const warp =
export function tile(w: Warp<"vec2"> | readonly Warp<"vec2">[], motif: (cell: Folded<"vec2">) => P3): Paint
```

## src/motion/body.ts
```ts
export type V3 = readonly [number, number, number];
export const foot = (at: V3) => ({ kind: "foot" as const, at });
export function neighbours(chains: readonly Chain[])
export function body(spec: Body, edges = neighbours(spec.chains))
```

## src/motion/move.ts
```ts
export function move(shape: Body | readonly Body[], source: string, inputs:
```

## src/motion/roam.ts
```ts
export function roam(laws: Partial<{ arch: string; step: string; present: string; deck: string }> = {})
```

## src/relief/displace.ts
```ts
export type ShellFrame =
export function shellFrame(s: Surface, u: F, v: F): ShellFrame
export function displace(s: Surface, height: Height): Surface
```

## src/relief/height.ts
```ts
export type Height = (u: F, v: F) => F;
export type Wrap = "none" | "u" | "v" | "uv";
export type Patch = { readonly size: readonly [number, number]; readonly wrap: Wrap };
export type UV = readonly [number, number];
export function offset(patch: Patch, u: F, v: F, a: readonly [F | number, F | number]): Expr<"vec2">
export function segmentDistance(patch: Patch, u: F, v: F, a: UV, b: UV): F
export function pathDistance(patch: Patch, u: F, v: F, path: readonly UV[]): F
export type Profile = "smooth" | "dome" | "cone" | "spike" | "crater" | "plateau";
export function profile(p: Profile, r: F): F
export type BumpSpec = { readonly at: UV; readonly radius: number; readonly height: number; readonly profile: Profile };
export function bump(patch: Patch, s: BumpSpec): Height
export type RidgeSpec = { readonly width: number; readonly height: number; readonly profile: Profile };
export function ridge(patch: Patch, path: readonly UV[], s: RidgeSpec): Height
export type CreaseSpec = { readonly width: number; readonly depth: number; readonly sharpness: number };
export function crease(patch: Patch, path: readonly UV[], s: CreaseSpec): Height
export type Region =
export function weight(patch: Patch, r: Region, u: F, v: F): F
export function flatten(patch: Patch, h: Height, region: Region, level: number): Height
export function inflate(patch: Patch, h: Height, region: Region, amount: number): Height
export const level = (h: number): Height => () => lift<"f32">(h);
export function sum(...hs: readonly Height[]): Height
export function smoothMax(k: number, ...hs: readonly Height[]): Height
export function clampHeight(h: Height, lo: number, hi: number): Height
export function scaled(h: Height, by: (u: F, v: F) => F): Height
```

## src/relief/sculpt.ts
```ts
export function sculptLayer(name: string, samples: ArrayLike<number>, size: readonly [number, number], o: { readonly wrap: Wrap }): Height
export function sampleGrid(fn: (u: number, v: number) => number, size: readonly [number, number], o: { readonly wrap: Wrap }): Float32Array
```

## src/relief/trace.ts
```ts
export type ShellRay = { readonly entry: P3; readonly dir: P3; readonly top: F };
export type Traced =
export const KIND = { miss: 0, ink: 0.5, hit: 1 } as const;
export function firstRoot(a: F, b: F, c: F): F
export type ShellModel =
export function shellModel(fr: ShellFrame, uv0: Expr<"vec2">, ray: ShellRay): ShellModel
export type MarchBudget =
export function traceMarch(fr: ShellFrame, uv0: Expr<"vec2">, ray: ShellRay, height: Height, b: MarchBudget): Traced
export function tracePlain(fr: ShellFrame, uv0: Expr<"vec2">, ray: ShellRay, height: Height, ink: F): Traced
```

## src/render/find.ts
```ts
export function find(w: ReturnType<typeof world>, samples: 1 | 4)
export function shadePixel(samples: 1 | 4, shade: string, background: string): string
export const identityFinish = "fn finish(c:vec4f,t:f32)->vec4f{return c;}";
```

## src/render/record.ts
```ts
export type V3 = readonly [number, number, number];
export type Thing =
export const thing = (shape: Thing["shape"], material: Thing["material"], at: V3 = [0, 0, 0]): Thing => (
export type World =
export type Scene = World &
export const scene = (s: Scene) => (
export type OrderedScene = ReturnType<typeof scene>;
export function world(s: World)
export function write(target: Write["target"], clear = 0, blend: Write["blend"] = "Opaque"): Write
export function raster(
export const screenDraw = (fragment: string): Draw => (
```

## src/render/twin.ts
```ts
export type Pick = { kind: "miss" } | { kind: "hit"; id: number; t: number; normal: V3; uv: readonly [number, number] };
export async function twin(scene: World, snapshots: Readonly<Record<string, Float32Array>> = {})
```

## src/render/covers/grid.ts
```ts
export function grid(u: number, v: number)
export function compact(law: string, indexCounts: readonly number[]): Source
```

## src/soft/cloud.ts
```ts
export type Rgb = readonly [number, number, number];
export type CloudSpec =
export type Cloud = { readonly puffs: readonly Puff[]; readonly bounds: Box };
export function hashUnit(seed: number, i: number, k: number): number
export function cloud(spec: CloudSpec): Cloud
export type PuffPlace = { readonly at: V3; readonly radii: V3; readonly density: number };
export type BillowSpec = Pick<CloudSpec, "turbulence" | "billow" | "drift" | "colour" | "seed" | "seconds">;
export function billows(places: readonly PuffPlace[], motion: BillowSpec): Cloud
```

## src/soft/density.ts
```ts
export type Rgb = readonly [number, number, number];
export type DensityOptions =
export type DensitySettings = Required<DensityOptions>;
export class DensityDrawable implements Drawable
export function densityIntegral(ctx: CompileContext, density: DensityDrawable): Shader & { readonly mixedCode: string }
export function boxVertexWgsl(b: Box): string
export function softRayWgsl(b: Box): string
```

## src/soft/gauss.ts
```ts
export const SIMPSON = 0.05;
export const FAR = 1e4;
export function erfcPos(z: number): number
export function gseg(a: number, b: number): number
export function gtail(a: number): number
export type Seg = { tau: number; tc: number; spread: number };
export function gaussRay(O: readonly number[], D: readonly number[], t0: number, t1: number): Seg
export function gaussDepth(O: readonly number[], D: readonly number[], t0: number, t1: number): number
export function gaussTail(O: readonly number[], D: readonly number[]): number
export const GAUSS_WGSL = /* wgsl */ `
```

## src/soft/glow.ts
```ts
export type Rgb = readonly [number, number, number];
export interface Strokes
export function strokes(name: string, count: number, segments: number, point: (u: F, i: F) => P3, taper: (u: F, i: F) => In<"f32"> = () => 1): Strokes
export type Veil = { readonly kind: "clear" } | { readonly kind: "medium"; readonly medium: Medium };
export type GlowOptions =
export type Shape = { readonly kind: "line" } | { readonly kind: "star"; readonly rays: number; readonly length: number; readonly thin: number };
export class GlowDrawable implements Drawable
export function glows(name: string, s: Strokes, o: GlowOptions): Drawable
export function glow(name: string, curve: (u: F) => P3, o: Omit<GlowOptions, "brightness"> & { readonly intensity: In<"f32">; readonly segments: number }): Drawable
export type FlareOptions = Omit<GlowOptions, "brightness" | "width"> &
export function flare(name: string, at: In<"vec3">, o: FlareOptions): Drawable
```

## src/soft/lightning.ts
```ts
export type StrikeSchedule = { readonly first: number; readonly every: number; readonly jitter: number; readonly seed: number };
export function strikeAt(s: StrikeSchedule, t: In<"f32"> = time): { readonly index: F; readonly since: F }
export function flicker(since: F): F
export function afterglow(since: F, seconds: number): F
export type BoltSpec =
export type Bolt =
export function displaced(u: F, a: P3, b: P3, levels: number, rough: number, key: F): P3
export function bolt(spec: BoltSpec): Bolt
export function boltFlash(b: Bolt, o: { readonly power: number; readonly colour: readonly [number, number, number]; readonly reach: number; readonly lift: number }): Light
export function sheetFlash(s: StrikeSchedule, box: { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] }, o: { readonly power: number; readonly colour: readonly [number, num
export function skyFlash(name: string, brightness: In<"f32">, colour: readonly [number, number, number]): Drawable
```

## src/soft/medium.ts
```ts
export type Blob = { readonly kind: "blob"; readonly centre: P3; readonly inv: P3; readonly density: F; readonly colour: P3 };
export type Layer = { readonly kind: "layer"; readonly height: F; readonly inv: number; readonly density: F; readonly colour: P3 };
export type Puff = Blob | Layer;
export type Bank =
export type Box = { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] };
export type BlobSpec =
export function blob(b: BlobSpec): Blob
export type LayerSpec =
export function layer(l: LayerSpec): Layer
export type BankSpec = LayerSpec &
export function bank(b: BankSpec): Bank
export const BLOB_LANES = 5;
export const CLUSTER = 8;
export const EXTENT = 3.6;
export type MediumTables =
export type Optional = { readonly kind: "none" } | { readonly kind: "some"; readonly table: TableSource; readonly count: number };
export class Medium
export function medium(name: string, parts: { readonly puffs?: readonly Puff[]; readonly banks?: readonly Bank[] }): Medium
export const GAUSS_LIB: LibFn = { name: "loft_gauss_ray", ret: "vec3", wgsl: GAUSS_WGSL, js: "gpu-only" };
export const BANK_LIB: LibFn = { name: "loft_bank_split", ret: "vec4", wgsl: BANK_SPLIT_WGSL, js: "gpu-only" };
export type MediumAccess =
export function accessMedium(m: Medium, sun: V3, lights: readonly Light[], table: (t: TableSource) => { readonly wgsl: string }): MediumAccess
export function clusterLoop(m: Medium, acc: MediumAccess, body: string): string
export function tauFunction(m: Medium, fn: string, acc: MediumAccess): { readonly code: string; readonly libs: readonly LibFn[] }
export function unionBox(boxes: readonly Box[]): Box
```

## src/soft/rain.ts
```ts
export type Rgb = readonly [number, number, number];
export type RainSpec =
export type SplashSpec =
export function rain(name: string, r: RainSpec): Drawable
export function splashes(name: string, r: RainSpec, s: SplashSpec): Drawable
```

## src/soft/specks.ts
```ts
export type Speck =
export type PointsOptions =
export function specks(name: string, count: number, speck: (index: F) => Speck, o: PointsOptions = {}): Drawable
```

## src/soft/specks_veil.ts
```ts
export function specksVeil(ctx: CompileContext, cloud: DensityDrawable, count: number): Shader & { cached: boolean }
export function specksVolumeVeil(ctx: CompileContext, v: VolumeDrawable, count: number): Shader & { cached: boolean }
```

## src/spacetime/attack.ts
```ts
export type Spine = (m: F) => P3;
export type Target =
export type Part = { readonly name: string; readonly body: Strand; readonly from: number; readonly to: number; readonly tip: number };
export function part(name: string, body: Strand, range: { readonly from?: number; readonly to?: number; readonly tip: number }): Part
export type Timing =
export type LungeSpec =
export type SweepSpec =
export type RemoteSpec = { readonly kind: "remote"; readonly hit: Hit };
export type GripSpec = Omit<GripperSpec, "rest" | "radius"> &
export type Motion = LungeSpec | SweepSpec | RearSpec | RemoteSpec | GripSpec;
export type BodyMotion = LungeSpec | SweepSpec | RearSpec;
export type Carry =
export type CarrySpec =
export type Instant = { readonly kind: "instant" };
export type Hold<N extends Ending = Ending> =
export type Effect = Instant | Hold;
export const INSTANT: Instant = { kind: "instant" };
export function hold<N extends Ending>(o: HoldSpec & { readonly end: N }): Hold<N>;
export function hold(o: HoldSpec): Hold<SetDown>;
export function hold(o: HoldSpec & { readonly end?: Ending }): Hold
export type Still = { readonly kind: "still" };
export type Ring = { readonly kind: "ring"; readonly speed: number; readonly height: number; readonly length: number; readonly reach: number; readonly hold: number; readonly drag: number };
export type Aftermath = Still | Ring;
export const STILL: Still = { kind: "still" };
export const ring =(o: Omit<Ring, "kind">): Ring => ({ kind: "ring", ...o });
export type AttackBeat = Beat & { readonly hold: F; readonly release: F; readonly flight: F };
export type Arming =
export const once = (at: In<"f32">): Arming => ({ kind: "once", at: lift<"f32">(at) });
export function every(period: number, first: In<"f32">): Arming
export type BodyAttackSpec<W extends Aftermath = Aftermath> = Common<W> &
export type RemoteAttackSpec<E extends Effect = Effect, W extends Aftermath = Aftermath> = Common<W> & { readonly motion: RemoteSpec; readonly effect?: E };
export type GripAttackSpec<H extends Hold = Hold, W extends Aftermath = Aftermath> = Common<W> & { readonly motion: GripSpec; readonly effect: H };
export type AttackSpec = BodyAttackSpec | RemoteAttackSpec | GripAttackSpec;
export type Envelope = { readonly p: F; readonly q: F; readonly tau: F };
export class Attack<M extends Motion = Motion, E extends Effect = Effect, W extends Aftermath = Aftermath>
export class BodyAttack<W extends Aftermath = Aftermath> extends Attack<BodyMotion, Instant, W>
export function attack<W extends Aftermath = Still>(spec: BodyAttackSpec<W>): BodyAttack<W>;
export function attack<H extends Hold = Hold, W extends Aftermath = Still>(spec: GripAttackSpec<H, W>): Attack<GripSpec, H, W>;
export function attack<E extends Effect = Instant, W extends Aftermath = Still>(spec: RemoteAttackSpec<E, W>): Attack<RemoteSpec, E, W>;
export function attack(spec: AttackSpec): Attack
export type Creature = { readonly things: readonly Drawable[]; readonly parts: Readonly<Record<string, Part>> };
export type Strike = { readonly attack: BodyAttack; readonly part: Part; readonly before: Spine };
export type Animated = Creature &
export function animate(rest: Spine, attacks: readonly BodyAttack[], build: (spine: Spine) => Creature): Animated
```

## src/spacetime/emote.ts
```ts
export type EmoteSpec =
export type EmoteBeat =
export class Emote
export function emote(spec: EmoteSpec): Emote
```

## src/spacetime/hit.ts
```ts
export type Capsule = { readonly kind: "capsule"; readonly a: V2; readonly b: V2; readonly radius: F };
export type Disc = { readonly kind: "disc"; readonly centre: V2; readonly radius: F };
export type Arc = { readonly kind: "arc"; readonly centre: V2; readonly inner: F; readonly outer: F; readonly from: F; readonly to: F };
export type Hit = Capsule | Disc | Arc;
export const hit =
export type Contour = { readonly d: F; readonly phase: F; readonly pitch: F };
export const polar = (a: In<"f32">): V2 => vec2(cos(a), sin(a));
export function hitSdf(h: Hit, p: V2): F
export function hitPatch(h: Hit, u: F, v: F, pad: F): V2
export function hitContour(h: Hit, p: V2, spacing: In<"f32">): Contour
export function hitInradius(h: Hit): F
export const laneOf = (h: Capsule): V2 => normalize(sub(h.b, h.a));
```

## src/spacetime/rear.ts
```ts
export type RearSpec =
export function rearSpine(a: Attack, m: RearSpec, rest: Spine): Spine
export function rearLanding(m: RearSpec): P3
```

## src/spacetime/routine.ts
```ts
export type Aim = V3 | "rest";
export type Who = "mirror" | { readonly canon: number } | { readonly only: readonly number[] };
export type Key =
export type Solo = { readonly solo: number; readonly on: readonly number[] };
export type RoutineSpec =
export type Dancer =
export type Readouts =
export class RoutineError extends SceneError {}
export class Routine
export type SyncRow = { readonly key: number; readonly count: number; readonly channel: string; readonly who: string; readonly arrival: number; readonly size: number; readonly pass: boolean; readonly arrivals: readonly n
export function routine(spec: RoutineSpec): Routine
```

## src/spacetime/smear.ts
```ts
export type SmearSpec =
export function smear(strike: Strike, spec: SmearSpec): Drawable
```

## src/spacetime/telegraph.ts
```ts
export type TelegraphStyle =
export const DANGER: TelegraphStyle =
export function telegraph(attack: Attack, o: { readonly style?: Partial<TelegraphStyle>; readonly name?: string } = {}): Drawable
```

## src/spacetime/wake.ts
```ts
export type WakeSpec =
export function wake(body: Body, spec: WakeSpec): Drawable
```

## src/style/biolume.ts
```ts
export function biolume(): Style
```

## src/style/foam.ts
```ts
export type FoamInk = "fill" | "line" | "edge";
export type InkLayers = readonly { readonly ink: FoamInk; readonly cover: F }[];
export function printLayers(under: Expr<"vec3">, layers: InkLayers, inks: Readonly<Record<FoamInk, Rgb>>): Expr<"vec3">
export type ContactFoam =
export type HeadSize = { readonly kind: "ofWidth"; readonly share: number } | { readonly kind: "world"; readonly radius: number };
export type MarksPrint = { readonly kind: "flat" } | { readonly kind: "heads"; readonly head: number };
export const foam =
export const marksAs =
export function contactFoam(p: ContactFoam, ctx: WaterCtx, lines: Pick<SwellLines, "off" | "live">): InkLayers
export function marksFoam(p: MarksPrint, ctx: WaterCtx): InkLayers
```

## src/style/gloss.ts
```ts
export function glossed(colour: V, ctx: ShadeCtx, o: { readonly gloss: In<"f32">; readonly metal?: In<"f32">; readonly seen?: (dir: V) => V }): V
```

## src/style/index.ts
```ts
export const STYLES =
export type StyleName = keyof typeof STYLES;
export const STYLE_NAMES = Object.keys(STYLES) as readonly StyleName[];
export function styleNamed(name: string): Style
export function withStyle(scene: Scene, style: Style): Scene
export function withLight(scene: Scene, name: string): Scene
```

## src/style/light.ts
```ts
export type Key = { readonly kind: "sun" } | { readonly kind: "offLens"; readonly least: number } | { readonly kind: "camera"; readonly side: number; readonly up: number };
export type Terminator =
export type Core = { readonly kind: "off" } | { readonly kind: "flat"; readonly edge: Terminator };
export type Highlight = { readonly kind: "off" } | { readonly kind: "shoulder"; readonly size: number; readonly ink: Rgb; readonly amount: number; readonly key: Key };
export type Cast = { readonly kind: "off" } | { readonly kind: "capsules"; readonly reach: number; readonly thin: number };
export type Waterline = { readonly kind: "off" } | { readonly kind: "band"; readonly height: number; readonly amount: number };
export type Overlap = { readonly kind: "off" } | { readonly kind: "band"; readonly width: number; readonly gap: number; readonly amount: number };
export type Bokashi = { readonly kind: "off" } | { readonly kind: "wash"; readonly foot: number; readonly under: readonly [number, number]; readonly amount: number };
export type Hatch = { readonly kind: "off" } | { readonly kind: "cross"; readonly edge: Terminator; readonly over: number; readonly deep: number; readonly cap: number; readonly spacing: number; readonly least: number; re
export type Rim = { readonly kind: "off" } | { readonly kind: "edge"; readonly width: number; readonly ink: Rgb; readonly amount: number; readonly key: Key };
export type Block = { readonly kind: "rule" } | { readonly kind: "ink"; readonly ink: Rgb; readonly amount: number; readonly value: number };
export type BlockInk = { readonly ink: Rgb; readonly amount: number; readonly value: number };
export type LightRecipe =
export const light =
export const LIGHT =
export type LightName = keyof typeof LIGHT;
export function keyOf(k: Key, ctx: ShadeCtx): V
export function lit(r: LightRecipe, ink: V, ctx: ShadeCtx): { readonly colour: V; readonly tone: F }
export type Bodies = Pick<Style, "shade" | "mark" | "block">;
export function litBodies(recipe: LightRecipe, own: BlockInk, ink: (ctx: ShadeCtx) => V, after: (c: V, ctx: ShadeCtx) => V = (c) => c): Bodies
export function blockInk(b: Block, own: BlockInk): BlockInk
export type Pull = { readonly floor: number; readonly add: Rgb };
export function pullOf(b: BlockInk): Pull
export const markOf = (p: Pull, tone: F): F => mix(p.floor, 1, tone);
export function printedBlock(p: Pull, colour: V, tone: F): V
export function castOf(c: Cast): BlockCast
export function overlapOf(o: Overlap): BlockOverlap
```

## src/style/neon.ts
```ts
export function neon(): Style
```

## src/style/paint.ts
```ts
export type Hue = "red" | "yellow";
export function standsOut(hue: Hue, c: Expr<"vec3">): Expr<"f32">
export type Warm = { readonly kind: "asPainted" } | { readonly kind: "hotter"; readonly margin: number; readonly gain: Rgb };
export type Mottle = { readonly kind: "off" } | { readonly kind: "blotches"; readonly size: number; readonly cut: number; readonly value: number; readonly margin: number };
export type LineWeight =
export const paint =
export function warmed(w: Warm, c: Expr<"vec3">): Expr<"vec3">
export function mottled(m: Mottle, c: Expr<"vec3">, ctx: ShadeCtx): Expr<"vec3">
export function weighted(l: LineWeight, c: Expr<"vec3">, ink: Rgb, ctx: ShadeCtx): Expr<"vec3">
```

## src/style/papercraft.ts
```ts
export const PAPERCRAFT_POST = { studio: { label: "papercraft studio", wgsl: STUDIO } } as const satisfies Record<string, PostSpec>;
export function papercraft(): Style
```

## src/style/pbr.ts
```ts
export function pbr(parts: Partial<PbrParts> = {}): Style
```

## src/style/pbr_parts.ts
```ts
export function ggx(n_h: F, rough: F): F
export function smithGgx(n_l: F, n_v: F, rough: F): F
export function schlick(v_h: F, f0: P3): P3
export function lambert(albedo: P3, n_l: F, _n_v: F, _v_h: F, _rough: F): P3
export function aces(c: P3): P3
export function daylight(dir: P3, sun: P3): P3
export type PbrParts =
export type PbrFailure =
export class PbrError extends SceneError
export function horizonClear(sky: SkyLaw, tone: (c: P3) => P3): readonly [number, number, number]
export function albedoOf(distribution: PbrParts["distribution"], visibility: PbrParts["visibility"]): PbrParts["lobeAlbedo"]
export function skyLight(at: { readonly normal: P3; readonly view: P3; readonly sun: P3 }, rough: F, sky: (dir: P3, sun: P3) => P3): { readonly diffuse: P3; readonly specular: P3 }
```

## src/style/post.ts
```ts
export function depthOfField({ focus, focalLength, fStop }: { readonly focus: number; readonly focalLength: number; readonly fStop: number }): PostSpec
export function filmGrain({ amount = 0.045 }: { readonly amount?: number } = {}): PostSpec
export function paperGrain(o: { readonly fibre?: number; readonly tooth?: number } = {}): PostSpec
export function bloom(o: { readonly radius?: number; readonly gain?: number; readonly from?: number; readonly to?: number } = {}): PostSpec
export function reflection(o: { readonly gain?: number } = {}): PostSpec
```

## src/style/sea.ts
```ts
export type SwellLines = { readonly cover: Expr<"f32">; readonly off: Expr<"f32">; readonly live: Expr<"f32">; readonly swell: Expr<"f32">; readonly calm: boolean };
export const calmSea = (ctx: WaterCtx): boolean => ctx.phase.node.op === "const";
export function swellLines(ctx: WaterCtx, o: { readonly perWave: number; readonly width: readonly [number, number] }): SwellLines
```

## src/style/shared.ts
```ts
export const rgb = (c: Rgb): Expr<"vec3"> => vec3(c[0], c[1], c[2]);
export const wgslRgb = (c: Rgb): string => `vec3<f32>(${c.map((x) => x.toFixed(4)).join(", ")})`;
export function edgeOf(x: Expr<"f32">, edge: number | Expr<"f32">, soft: number, widest = 0.05): Expr<"f32">
export function pixelsInside(ctx: ShadeCtx): Expr<"f32">
export const POST_COMMON = /* wgsl */ `
```

## src/style/sheet.ts
```ts
export const SHEET_CAMERA: Camera = { kind: "perspective", eye: [12.5, 12.5, 14.5], target: [0.3, 1.2, 0], fovY: 0.6 };
export function grid(frames: readonly Frame[], columns: number): Frame
```

## src/style/sketch.ts
```ts
export function sketch(): Style
```

## src/style/sky.ts
```ts
export type Wash = { readonly ink: Rgb; readonly from: number; readonly to: number };
export type SkySun = { readonly kind: "none" } | { readonly kind: "disc"; readonly toward: readonly [number, number, number]; readonly radius: number; readonly ink: Rgb };
export type SkyClouds =
export type Sky = { readonly horizon: Rgb; readonly washes: readonly Wash[]; readonly below: Rgb; readonly sun: SkySun; readonly clouds: SkyClouds };
export function skyOf(sky: Sky, ctx: SkyCtx): Expr<"vec3">
```

## src/style/toon.ts
```ts
export type ToonSpec =
export function toon(spec: ToonSpec = {}): Style
```

## src/style/whitewater.ts
```ts
export type Lean = { readonly kind: "none" } | { readonly kind: "toward"; readonly dir: V2 };
export type WhiteOn = { readonly xz: V2; readonly mass: F; readonly slope: V2; readonly lean: Lean };
export const slopeOf = (m: In<"f32">, xz: In<"vec2">): Expr<"vec2"> => lib<"vec2">(SLOPE_OF, m, xz);
export type White = { readonly fill: F; readonly line: F };
export type Roll = { readonly head: number; readonly line: number; readonly held: number };
export function rollOf(head: number, held = 0.75): Roll
export const along = (xz: V2, degrees: number, period: number): F =>
export const pixelOn = (xz: V2): F => max(max(fwidth(xz.x), fwidth(xz.y)), 1e-5);
export function rollingFoam(on: WhiteOn, r: Roll): White
export function curledHeads(on: WhiteOn, r: Roll): White
export function crown(d: V2, rim: F, reach: F, count: number, r: Roll): White
export function behind(back: White, front: White): White
export function joined(a: White, b: White): White
```

## src/style/woodblock.ts
```ts
export const WOODBLOCK_PALETTE =
export const WOODBLOCK_SEA = { deep: [0.11, 0.18, 0.35], mid: [0.16, 0.26, 0.45], line: [0.58, 0.68, 0.78], haze: [0.7, 0.76, 0.78] } as const satisfies Record<string, Rgb>;
export type WoodblockInks = { readonly black: Rgb; readonly indigo: Rgb; readonly pale: Rgb; readonly cream: Rgb };
export type AccentInk = { readonly ink: Rgb; readonly from: number };
export type Accent = { readonly hue: Hue; readonly margin: number; readonly inks: readonly AccentInk[] };
export type Print =
export type InkKey = { readonly key: Rgb; readonly shadow: Rgb; readonly lit: Rgb };
export const print =
export type WoodblockEye = { readonly kind: "none" } | { readonly kind: "ink"; readonly below: number; readonly amount: number };
export type SeaPalette =
export const seaPalette =
export type WoodblockSpec =
export function woodblock(spec: WoodblockSpec = {}): Style
export const REFERENCE_PAINT: NonNullable<WoodblockSpec["paint"]> = { warm: paint.hotter(), mottle: paint.mottle(), line: paint.swelling() };
export type WoodblockLook = { readonly about: string; readonly on: (s: WoodblockSpec) => WoodblockSpec };
export const LOOKS =
export type LookName = keyof typeof LOOKS;
```



================================================================
# !!IMPORTANT!! KEY INSTRUCTIONS
# file: docs/handoff/2026-11-20_spec/KEY_INSTRUCTIONS.md
# what it is: how to treat our prior conversations and exactly what to send back
================================================================

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
