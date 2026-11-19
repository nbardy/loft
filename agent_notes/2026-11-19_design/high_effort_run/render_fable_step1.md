# Render, step 1: where current engines fall short of Loft's render goals

Fable, 19 Nov 2026. Step 1 of three (critique; then invent and architect; then implement). This page judges prior
art and Loft itself against `docs/PHILOSOPHY.md`, `docs/design/render.md` and `CORE.md`, and nothing else. It
proposes no design. Section 6 lists what to throw out and section 7 the questions step 2 has to answer.

Numbers are marked **[repo]** (read today in this repository), **[recalled]** (from my memory of the paper, talk or
documentation; I opened no outside source in this step, so re-check before building on the exact figure) or
**[derived]** (my arithmetic, shown). I did not read the sibling critique; it was empty when I started.

## 1. The yardstick

The design goal, cut into eight tests. Every critique below names the tests it fails.

| | Test |
|---|---|
| G1 | One frame mixes analytic intersection, distance fields, triangles, traced rays, volumes and particles, chosen per object, sharing one depth, one light transport and one material model |
| G2 | Light converges to correct: many lights, shadows from every method, global illumination and reflections, temporal reuse, and a path-traced reference the real-time picture is judged against |
| G3 | Materials at the state of the art, each lobe earning its cost and each one replaceable |
| G4 | Frame time, p50 and p95, on the target GPUs |
| G5 | Load time does not grow with content |
| G6 | The method is chosen per site by measured cost; the author's code does not change when the choice does |
| G7 | A scene is a few lines; a new primitive, BRDF, estimator or pass is a few lines more and plugs into the same depth, lights and materials |
| G8 | No baked engine: a core of hundreds of lines, every built-in a public composition, nothing privileged, everything printable, nothing reported that is false |

My scoring, to be argued with (F fails, P partial, Y passes):

| System | G1 | G2 | G3 | G4 | G5 | G6 | G7 | G8 |
|---|---|---|---|---|---|---|---|---|
| Unreal 5 | F | P | Y | P | F | P | F | F |
| Unity HDRP | F | P | Y | P | F | F | F | F |
| Godot 4 | F | P | P | P | P | F | P | F |
| Bevy | F | P | P | P | P | F | F | P |
| three.js, Babylon | F | F | P | F | P | F | P | F |
| Filament | F | F | Y | Y | Y | F | F | F |
| Shadertoy and 4k intros | F | F | F | F | P | F | P | P |
| Dreams, Teardown | F | P | F | Y | P | F | F | F |
| PBRT, Mitsuba | P | Y | Y | F | F | F | F | P |
| **Loft today** | **P** | **F** | **F** | **P** | **F** | **F** | **P** | **P** |

Nothing passes G1 or G6; that is where the ambition is. Loft's own row is no better than the engines it wants to
replace. It is differently bad, and section 5 says how.

## 2. The seven root failures

Read across the prior art and the same seven mistakes recur. Each engine below is an instance of several.

**R1. The scene exists several times, because each question has its own representation.** Unreal stores one object
as Nanite clusters (to rasterise), a mesh distance field (for Lumen's software rays), surface-cache cards (for
Lumen's lighting), and a simplified mesh in a BLAS (for hardware rays): four lossy copies, none of which is what the
object *is*. A sphere is a centre and a radius, and all four questions have closed-form answers on it. The copies
exist because a triangle soup carries no knowledge of its own shape.

**R2. The unit of extension is a named kind, and going past the menu means changing language.** A shading model is
an enum, a light is a class, a pass is an injection point, a primitive is a mesh. Past the menu the consumer forks
the engine in C++ or patches shader strings. `docs/open_design.md` says this of authoring APIs; it is equally true of
the renderer's internals.

**R3. Composition is done by shader permutation.** Features combine through preprocessor keywords, so the number of
compiled programs is a product: materials times lights times passes times vertex formats. Load time and hitching
then grow with content (G5). Unreal's "compiling shaders" counter and pipeline-state stutter, Unity's variant
stripping, three.js's define strings and Loft's 305 modules for five crabs are one disease.

**R4. Light transport is welded to the frame's layout.** The estimator is written against a fixed G-buffer, a fixed
material enum and a fixed list of light kinds. ReSTIR is, mathematically, a short function over a sample, a target
function and a reservoir; it ships as an SDK that takes months to integrate. Nobody can replace "the light
transport" because it is not a part; it is the shape of the whole frame.

**R5. Temporal accumulation without a reference.** Real-time engines hide noise behind history (TAA, denoisers,
probe hysteresis) and cannot say how far the result is from correct; ghosting, lag and leaks are tuned by eye.
Offline renderers have the reference and no real-time path. Nobody ships both from one description with a number
between them. Unreal and HDRP come closest: their path tracers read the same assets, through different code.

**R6. The author makes the performance decisions by hand.** Which objects get LODs or a distance field, which
light casts shadows, which pass runs at half resolution: flags and assets set by a person, re-authored to change.
Nanite is the great counter-example (detail chosen per cluster by screen error, no author input), which is why it
mattered more than any shading feature of its decade.

**R7. Size.** Unreal is millions of lines; HDRP, Bevy's renderer and three.js are each around a hundred thousand
**[recalled, order of magnitude]**. No part is "rewritten in minutes" when understanding its neighbours takes a
week. Size is also why these engines cannot fix R1 to R6: a million lines depend on each.

## 3. Prior art, one at a time

### 3.1 Unreal Engine 5 (Nanite, Lumen)

**What it does wrong.**

- *Triangles only* (G1). Nanite draws clusters of up to 128 triangles from a hierarchy built offline, into a
  visibility buffer, rasterising small triangles in compute with 64-bit atomics **[recalled: Karis 2021]**. It is the
  best answer to "draw any triangle soup" and the wrong answer to "draw a capsule", which is one quadratic per
  covered pixel with an exact silhouette and normal. Unreal has no analytic primitive and no authored distance
  field: its fields are baked *from* triangles, as proxies.
- *Lumen is an approximation stack over proxies* (G2). Software Lumen traces mesh distance fields near the pixel and
  a global distance field beyond, reads lighting from a surface cache of cards, and gathers from screen-space probes
  placed about every 16 pixels with 64 directions each, so about a quarter of a ray per pixel, hidden by temporal
  filtering **[recalled: Wright 2022]**. Skinned meshes are absent from the software scene. Thin walls leak. Mirror
  reflections need the hardware path. Indirect light lags a fast change by several frames.
- *Materials compile to permutations* (G5, R3). A material graph becomes HLSL compiled once per combination of
  usage flags, vertex factory and pass. Epic has written publicly about pipeline-state stutter **[recalled]**.
- *The extension unit is the engine source* (G7, G8). A new shading model is an enum member plus edits to engine
  shaders and C++ in about a dozen places, then an engine rebuild **[recalled]**. G-buffer and pass order are fixed.
- *Mandatory cook* (G5). Cluster and distance-field builds and shader compilation precede the first frame and grow
  with content.

**Why.** Its customers bring assets from modelling tools. A triangle soup has no closed form, so every method had
to be brute force made uniform. Twenty-five years of C++ and a visual material editor fixed the permutation model
long before a compiler could fuse a frame.

**What it costs.** Speed: about 4.5 ms for Nanite's visibility and material passes in Epic's 2021 PS5 demo, and a
Lumen budget near 4 ms at 1080p internal for 60 Hz on consoles **[recalled]**: excellent for triangle soup, far
above what an analytic scene needs. Memory: four copies (R1). Developer experience: changing the light transport is
a multi-week engine modification. Tokens: nothing about the renderer fits in a model's context.

**Throw out.** The triangle-only premise and the proxy stack that follows from it. Material graph to permutation.
The shading-model enum. The fixed G-buffer. The cook as a precondition.

**Keep.** The visibility buffer's separation of "what is at this pixel" from "how it is shaded". Screen-error
level of detail with no author input. The screen-probe gather as one valid estimator. Having a path tracer in the
same product at all.

### 3.2 Unity HDRP and Godot 4

**Unity, wrong.** "Scriptable" in name: HDRP is a very large body of C# and HLSL that nobody outside Unity
rewrites, so extension happens at a handful of fixed injection points (after opaque depth and normals, before
transparents, before and after post). The Lit shader is one shader with feature flags. Keyword permutations reach
counts that need a build-time stripping system **[recalled]**. Three incompatible pipelines (built-in, URP, HDRP)
mean a material written for one does not run on another.

**Godot, wrong.** Three fixed renderers. A shader is a restricted language with fixed entry points (`vertex`,
`fragment`, `light`); the lighting loop, pass list and buffers are closed unless you edit C++. Global illumination
is a menu (SDFGI, VoxelGI, lightmaps), each with its own leaks.

**Why.** Unity had to serve phones and consoles from one editor and split the renderer instead of making the
method a per-device choice over one description; keywords were the composition mechanism of 2010. Godot hides the
pipeline on purpose, for small teams without a graphics programmer.

**Cost.** G5 (variants), G7 (a custom pass is a class, a volume component and a shader, in two languages), G8
(injection points are the definition of a privileged default), G1 (no analytic primitives; distance fields only as
a GI proxy).

**Throw out.** Keyword permutations, injection points, closed pipelines, and a pipeline per device class. **Keep**
the lesson that defaults must make the first scene trivial.

### 3.4 Frostbite and id Tech

Two proprietary monoliths that each got one thing right.

**Frostbite** gave the field the frame graph (passes declare what they read and write; the graph culls unused
passes and aliases transient memory) and the most careful public account of physical units, area lights and
energy-conserving diffuse **[recalled: O'Donnell 2017; Lagarde and de Rousiers 2014]**. Wrong: a frame graph
treats each pass's shader as an opaque box, so it schedules memory and order but cannot fuse, specialise or move
work between passes, which PHILOSOPHY 2 asks for.

**id Tech 7 and 8** are the speed lesson: clustered forward shading with a few hundred pipeline states where a
permutation engine has tens of thousands **[recalled]**, everything GPU-driven, no per-object CPU work. Uniform data
paths beat clever per-object code. Wrong: the uniformity is one studio hand-tuning one shader set for one kind of
game. The consumer can replace nothing.

**Throw out** both monoliths. **Keep** declared passes, physical units, and "few programs, much data".

### 3.5 Bevy

The nearest in spirit: modular, open, Rust, on wgpu, with an experimental cluster renderer and an experimental
ReSTIR path.

**Wrong.**

- *Modularity at the wrong level.* The unit is a plugin: ECS systems across extract, prepare and queue, a
  render-graph node, a pipeline specialised by bit-flag key, bind-group layouts. A custom render phase in Bevy's own
  examples runs to several hundred lines **[recalled]**, almost none about the picture.
- *Shader composition by text import and `#ifdef`* (R3). Pipelines are specialised at run time by key, compiled
  asynchronously, and pop in.
- *Per-frame CPU work proportional to entities* (extraction into the render world), which its own author names as
  the bottleneck once rasterisation is fast: culling at 1.27 ms against 0.42 ms for the raster pass **[repo:
  `docs/tracing_sota.md` 4.3, read from the author's post]**.
- *Triangles only* (G1).

**Why.** It is an ECS game engine first. The renderer had to be expressed in the ECS's terms, and wgpu's explicit
layouts were surfaced rather than derived.

**Cost.** G7 above all: tokens. And churn: the API changes every few months.

**Throw out.** Pipeline keys. Plugin registration as the meaning of "modular". Hand-written bind-group layouts.
**Keep.** wgpu, a device the host owns, and the proof that cluster rendering works on it.

### 3.6 three.js and Babylon.js

**Wrong.** A mutable scene graph traversed on the CPU every frame, one draw call and one uniform upload per object,
so the practical ceiling is a few thousand objects regardless of triangle count (G4). Materials are classes; the
classic escape hatch, `onBeforeCompile`, is a string replace against the library's internal shader chunks, which
breaks when the chunks change (R2 in its purest form). TSL is the right repair for shading and stops at the
material's slots (`docs/vs_threejs.md` [repo]). No global illumination. Transmission samples a blurred copy of the
opaque scene, so transmissive objects do not refract each other. Transparency is sorted per object by centre.
Shaders compile on first use and hitch.

**Why.** Designed in 2010 to make WebGL 1 feel like a fixed-function scene API. Every later capability was fitted
to that object model.

**Cost.** G1, G2, G4. Its G7 score is partial rather than failing only because a first scene really is short.

**Throw out.** The CPU scene graph in the frame loop. Material classes. String-patched shaders. **Keep.** The
thirty-line first scene as the bar for defaults, and the asset reach (glTF in one call), which Loft lacks entirely.

### 3.7 Filament

**Wrong.** The best-documented real-time material model there is, and a closed one. A material is a small function
that fills a fixed struct of inputs for one of a fixed list of shading models, compiled offline by `matc` into a
package of variants. The BRDF, the lighting loop, the froxel light culling and the pass order are C++ the consumer
does not touch. No global illumination beyond image-based light with screen-space occlusion and reflection.

**Why.** Built for phones, where a fixed, measured path is the product. That is why it passes G4 and G5 on its own
terms.

**Cost.** G1, G2, G8.

**Throw out.** The offline material compiler and the closed shading-model list. **Keep.** Its material maths as the
audit baseline for G3 (energy compensation, clearcoat, sheen, anisotropy, the cloth model), and its habit of stating
each term's derivation and limit.

### 3.8 The demoscene, Shadertoy, Inigo Quilez

This tradition is the origin of half of G1, so its failures matter most.

**Wrong.**

- *One function is the whole scene.* `map(p)` unions every part, and every march step of every pixel evaluates it.
  Cost is pixels times steps times parts. That is the wall at a few dozen primitives. Lipschitz pruning's result
  says how much is wasted: with a spatial structure, about 1.0 to 1.2 parts remain per cell, and a 1,989-part crowd
  renders 227 times faster **[repo: `docs/tracing_sota.md` 4.1, read from the authors' slides]**.
- *Secondary queries multiply it.* A normal by finite differences is four to six more evaluations of the whole
  scene; ambient occlusion five; a soft shadow sixteen to sixty-four. A typical pixel pays hundreds of whole-scene
  evaluations **[derived from common shader structure]**. A symbolic gradient or an analytic normal costs about
  one.
- *The light is fake and cannot converge.* The penumbra from the closest approach along a shadow march, the
  five-tap occlusion, the fog: each is a beautiful hack with no estimator behind it and no limit it approaches (G2).
- *Nothing composes.* No depth shared with triangles, no assets, no per-object bound, so rays that miss still
  march (75.5% of the work on Loft's own tree model **[repo: `docs/tracing_sota.md` 3]**).
- *Compile time.* Large fragment shaders take tens of seconds to compile on some backends **[recalled]**; the scene
  is the shader, so load grows with content here too (G5).

**Why.** A 4 KB size limit and a platform that offers one fragment shader and no vertex stage, compute or buffers.
The style is an adaptation to a constraint we do not have.

**Cost.** G1 (one method), G2, G4, and G7 at scale: fifty things are one long function, not fifty short pieces.

**Throw out.** The global `map`. Finite-difference normals as the default. Hack shadows and occlusion as the light
transport. "Everything is marched" as an identity.

**Keep.** The analytic intersectors and distance functions, domain repetition with cell ids, smooth blends, maths
written as maths, zero assets, and the taste.

### 3.9 Dreams and Teardown

Both bet on one unusual representation and both prove the same point from opposite sides.

**Dreams** authors with distance-field edits and renders none of them directly: edit lists are evaluated into point
clouds and drawn as splats with heavy temporal accumulation **[recalled: Evans 2015]**. So the authoring language is
analytic and the picture is not: hard edges, exact silhouettes and crisp reflections are given up. Wrong against G1 (everything becomes one representation) and G8 (a closed
console engine). **Keep** the insight Loft's own research already reached (`docs/tracing_sota.md` 5.4 [repo]): the
field is the modelling language and need not be the drawing schedule.

**Teardown** makes everything voxels in small volumes, rasterises their boxes, marches the volume per pixel, and
keeps one world-space occupancy volume that shadows, occlusion and reflections all trace **[recalled: Gustafsson's
write-ups]**. Because every light query sees one uniform structure, it had ray-traced soft shadows and reflections
on mid-range hardware without ray-tracing hardware. Wrong against G1: the price of that uniformity was throwing away
every smooth surface. **Keep** the lesson, the positive form of R1 and R4: light transport became cheap and general the
moment every object answered the same ray question. The goal is that property without the single representation.

### 3.10 ReSTIR and RTXDI, DDGI

**ReSTIR** is the most important light-transport result of the period: resampled importance sampling with
spatial and temporal reuse renders millions of dynamic emitters with about one shadow ray per pixel, 6 to 60 times
faster than its predecessors at equal error, more in its biased form **[recalled: Bitterli et al. 2020]**; ReSTIR GI
and path-space ReSTIR extend it to indirect light.

**Wrong, as delivered.** R4. It ships as vendor SDKs that assume hardware rays, a G-buffer with a fixed material
model, motion vectors, and a closed denoiser, now often a neural one nobody can read (G8). Temporal reuse brings
correlation artefacts and lag whose size nobody reports (R5). A consumer who wants a different target function,
reuse neighbourhood or light kind edits an SDK, not a function.

**DDGI** places irradiance probes on a grid, each a small octahedral map of irradiance (8x8 texels) and of distance
(16x16), updated by a hundred or so rays per probe per frame and blended with a hysteresis near 0.97 **[recalled:
Majercik et al. 2019]**. Wrong: diffuse only; detail limited to the probe spacing; leaks through thin walls despite
the distance test; a hysteresis of 0.97 means a change of light takes on the order of a hundred frames to settle
**[derived: 0.97^n < 0.05 at n = 99]**; and probe placement is a manual art (R6).

**Why.** Both were built to be dropped into existing deferred engines, so they took those engines' buffers as given.

**Throw out.** The SDK shape. The mandatory closed denoiser. Hysteresis tuned by eye with no reference.

**Keep.** The mathematics: reservoirs and resampling as a small generic function; reuse across pixels and frames as
a schedule over that function; visibility-weighted probe interpolation. These are laws in PHILOSOPHY 3's sense and
should be about as long as their equations.

### 3.11 Hardware ray tracing

**Wrong.** The acceleration structure is a black box with a triangle-first interface. Custom primitives exist as
boxes with an intersection shader, on a slower path, with no rasterised counterpart, so they are second-class (G1).
The structure cannot be shaped by the application, which is why Nanite geometry could not be traced directly for
years and engines kept a second, simplified scene for rays (R1). Deforming geometry needs a refit or rebuild per
frame. The API surface is vendor- and platform-specific, absent from browser WebGPU, and experimental in wgpu
**[repo: `docs/proposals/headroom.md` section 2]**.

**Why.** It was standardised around what film and game assets already were.

**Cost.** For us, mainly reach and G1.

**Throw out.** The assumption that "ray tracing" means that API. A ray against analytic primitives and bounded
fields is a question a compute or fragment shader answers today. **Keep** hardware traversal as one schedule for it.

### 3.12 PBRT, Mitsuba 3, Falcor, Slang

**PBRT** defines what correct means, and its GPU path shares code with its CPU one. Wrong for us: shapes, materials
and lights are closed lists dispatched through tagged pointers, so a new kind means editing the list and
recompiling (R2); no real-time path.

**Mitsuba 3** is the closest prior art to PHILOSOPHY 2. Dr.Jit traces the renderer's own code and compiles the
whole composition, virtual calls included, into one kernel per configuration, and differentiates it. That is
"compile the composition, not the components", working, in a shipped system. Wrong for us: kernels take seconds to
minutes to compile, which is load time growing with composition; no rasteriser, no hybrid, not real-time.

**Falcor** has the right research shape (passes in a graph, Slang shaders, a path tracer beside real-time passes)
inside a large Windows-first C++ framework whose scene and material library is a monolith.

**Slang** fixes permutations at the language level: interfaces and generics specialised by the compiler, with
dynamic dispatch as the alternative, plus automatic differentiation. It is the best existing answer to R3 as a
*language*. Wrong for us: it is a language and a large compiler, not a system. It does not know rates (its research ancestor,
Spire, had them **[recalled]**), cannot evaluate on the CPU, cannot bound a function over a region, does not
schedule passes, and leaves specialise-or-dispatch to the programmer (G6).

**Throw out.** Closed kind lists. Whole-program megakernels as the only compilation unit. A new shading language
when WGSL text with holes already lowers into a graph Loft can read five ways.

**Keep.** PBRT's definition of correct. Mitsuba's proof that a traced composition can be fused and differentiated.
Slang's proof that specialise-or-dispatch is a compiler decision.

### 3.13 WebGPU and wgpu

Not an engine, but its limits have shaped Loft more than any engine has, so they belong here.

The limits **[recalled from the specification; confirm against the pinned wgpu]**: 8 storage buffers per shader
stage and 4 bind groups by default; no ray tracing; no mesh shaders, multi-draw indirect or bindless in the core
browser standard (wgpu has native-only forms of some); 32-bit atomics only, so the visibility-buffer trick of
packing depth and payload into one 64-bit atomic is unavailable; every shader is compiled twice (WGSL to the backend
language, then by the driver); no application-controlled pipeline cache in the browser.

**What is wrong is not the limits but designing down to them.** Loft treats "nothing may depend on hardware rays"
as a rule of the design (headroom.md section 2 [repo]). The goal says the opposite: one description, the method
chosen by what the device offers and what it costs: compute traversal and fragment-stage intersection in the
browser, ray queries on native where they pay, the scene unchanged. **Throw out** the lowest common denominator as a design rule. **Keep** it as one target among several.

## 4. The numbers that carry the argument

| Fact | Number | Kind |
|---|---|---|
| Five crabs in Loft: modules, WGSL lines, cold load, then GPU frame | 305, 1.13 million, 55 s, then 4.26 ms | repo (`thirdparty_fable.md` 4) |
| Coral: marched, then listed and intersected | 4.9 to 6.8 ms, then 1.3 to 1.5 ms | repo (`tracing_sota.md` 5.3) |
| Lipschitz pruning, 1,989-part crowd | 227 times faster than unpruned | repo (read from slides) |
| Loft: one more light | about 0.05 ms, cap of 7 | repo (`docs/lighting.md`) |
| ReSTIR direct light | about one shadow ray per pixel, whatever the light count | recalled |
| Loft PBR lobe albedo, face on, roughness 1 | 0.31 | repo (`src/style/pbr_parts.ts`) |

Two derived lines matter most. Loft's light cost is linear in lights times lit fragments: 1,000 lights at 0.05 ms
is 50 ms, where the state of the art is constant in light count. And 305 modules in 55 s is 180 ms a module; if
modules followed kinds of thing (perhaps 15 for a crab scene) the same rate gives under 3 s cold, and a disk cache
brings it near zero **[derived]**.

## 5. Loft as it is today

Loft is judged hardest because it is the one we can change. Facts are from the repository at `dfc6dd8`.

### 5.1 What it gets right, and no other system here has

- One typed graph read several ways: GPU code, a CPU twin, a symbolic gradient, intervals. This is the answer to R1
  nobody else has: one description of what a thing is, many questions asked of it.
- Rates found by analysis; the schedule separate from the description (a field may be traced, bricked or meshed).
- Time as an argument, so any frame draws alone and motion blur is exact.
- The loaded file is already a frame graph as data, and a second host exists.
- Typed refusals that name the fix; checks that say holds, fails or undecided.
- Research ahead of the engine: `docs/tracing_sota.md` derives unfolding, solving onto the body, hull marching and
  an exactly local blend, with counts. Only unfolding is built.

### 5.2 G5 fails: load time grows with content

Each drawn thing gets its own generated shader module. Five crabs: 305 modules, 1.13 million lines of WGSL, 55 s
before the first frame, against a steady frame of 4.26 ms. The Titan records 308k nodes because every call is
inlined at record time.

*Why.* Specialisation was the only composition mechanism: shaders were built by concatenating strings per thing
(about 200 interpolations in `src/soft` alone), and a later pass re-parses the text to merge modules that differ
only in literals. The byte-identical gate then made every merge risky. This is R3, reached by a different road.

*Cost.* The one failing that makes Loft unusable at scale, and it poisons G7: a minute's wait to see an edit.

*Throw out.* A module per thing, string-built shaders, inlining at record time. CORE.md already says so; the Rust
plan's templates and call graph point the right way.

### 5.3 G2 fails: there is no light transport

What exists [repo: `docs/lighting.md`, `src/draw/shadows.ts`, `src/light.ts`, `src/style/pbr.ts`]: one sun; at most
seven other lights, each a term in every lit fragment; the sun's cast shadows are skeleton capsules projected on the
ground only, and bodies do not shadow each other or themselves under the sun (visibility is 1); a lamp may have one
shadow map; occlusion is four taps along the normal, for fields only; reflection is a tap of the closed-form sky
plus a screen-space mirror for water; `castRay` sees capsules. No global illumination, no many-light sampling, no
reflections of one object in another, no acceleration structure of any kind, no reference renderer.

Worse is where the little that exists lives. **Light is inside the style.** Each of seven styles "prints" lights
its own way and supplies its own `water`, `shadow`, `ground` and `sky`. No layer under the looks computes
visibility, irradiance or radiance for all of them, so "one light transport" has nowhere to go, and "stylised looks
are compositions of the same pieces" is false today: they are siblings that each re-implement the frame.

And the written design forbids the goal. `docs/proposals/headroom.md` section 10 lists as "never chase": temporal
reprojection of any kind, real-time path tracing and global illumination, deferred shading, hardware ray tracing,
and physically based exposure and tone-mapping pipelines. The reconciled proposals rule that physically based light
is "a separate offline tool" and that nothing is carried from one frame to the next. render.md now demands temporal
reuse and denoising, global illumination and reflections in real time, hardware rays where they pay, exposure and
tone mapping, and a path-traced reference mode. Those rulings are superseded and should be marked so.

*Why.* Loft was built for stylised creatures on a sea, in nine days, under two invariants right for that job: any
frame draws alone, and pixels are byte-identical. Every technique that needs history or noise violated one. The
estimate behind "never" (30 to 80 ms per path-traced sample on the largest scene) assumed brute-force paths with no
reuse, on one laptop GPU.

*Cost.* The whole of "Light and beauty". CORE.md's first priority is the picture.

*Throw out.* "Nothing carries over between frames" as an invariant of the engine; keep it as a property of one mode
(the reference still), which is what makes that mode a reference. Byte-identical pixels as a gate (already replaced
by picture parity). The cap of seven and lights as distinct code. Skeleton capsules as the shadow system. Light
living in the style.

### 5.4 G3 fails: one basic lobe, and the picture is display-referred

The PBR style is GGX with height-correlated Smith visibility, Schlick Fresnel, a Lambert body and an analytic
split-sum environment. Its own comments list what it cannot do: no multiple-scattering compensation (the lobe's
face-on albedo is 0.31 at roughness 1), no anisotropy; and there is no clearcoat, sheen, transmission, subsurface or
thin film in the default. CORE.md's status line is honest: "gap to the state of the art not assessed".

The frame underneath cannot hold a physically based picture anyway. The view target is `rgba8unorm`,
display-referred, always 4x multisampled; the PBR style linearises and tone-maps inside its own shade function. So
bloom, depth of field and exposure operate on tone-mapped 8-bit values, and still mode averages samples already
tone-mapped and quantised (`src/host/still.ts` header): a bright highlight blurs to the wrong brightness in the very
mode meant to be the quality reference. No image texture can be read, so there are no measured materials and no
captured environments.

*Why.* PBR arrived as the seventh style on a frame designed for ink.

*Throw out.* The display-referred frame. Tone mapping inside a material. Mandatory 4x MSAA. PBR as "a style".

### 5.5 G1 is partial: four methods share depth, none share light, and the core's interface is a look

Loft does mix rasterised parametric surfaces, sphere-traced fields, analytic impostors, Gaussian and marched media
and particles in one depth buffer. That is real and rare. What is missing:

- Traced rays against anything but capsules; any spatial lookup for many-part fields (CORE: "not yet"), so a field
  of N parts is still an N-way `min` per step, the demoscene's wall.
- Imported triangles and textures; ordered or order-independent transparency (draws go by phase and list order).
- A shared answer to "what does a ray from here hit". Each tracer finds first hits for the camera only.
- Readback: the Rust host's stated rule is "inputs in, pixels out, nothing back" (`loft-gpu/src/lib.rs`), where
  render.md asks for depth, ids and picking out.

The deeper problem is in the planned core. The lean Rust plan's ten types are a good 2x cut, and they are still a
baked engine, because the interface between the core and the library encodes this renderer's content:

- `paint (p, n, id) -> vec4`, "alpha below 0.5 asking the composer for ink" (`loft-core/src/draw/mod.rs`): the
  woodblock outline protocol is in the signature of every surface in the engine.
- `hit (o, d, span, width) -> vec4` returns a normal and a distance and nothing else: no material coordinates, no
  primitive id, no second root. A reflection or a path cannot ask what it hit.
- `Layer { Subject, Surroundings }`, `Compose` with ground and outline, `Target::Shadow { light, map }`,
  `Shade { paint, style }`, `Medium` with fixed albedo, phase and light: scene content as core enums.
- `Cover` and `Find` are closed lists. A consumer's new way to cover pixels (a compute rasteriser, a splat binner, a
  visibility buffer) is not a variant.

PHILOSOPHY 1 says the core loads programs, resources and an ordered pass list, in hundreds of lines. Against that,
the ten types are the *default engine* and belong in the library, built from a smaller thing.

*Why.* The plan was written the same morning as the philosophy, as a port of what existed.

*Throw out.* Ink, ground, sea, sun, style, layer and shadow as concepts the core knows. The fixed `hit` and `paint`
signatures. Closed `Cover`. "Nothing back".

### 5.6 G6 fails: the author chooses the method

Schedules are separate from descriptions, but a person picks one by calling `draw.field`, `draw.copies`,
`draw.traced` or `draw.surface`. The coral went from 4.9 to 6.8 ms down to 1.3 to 1.5 ms by rewriting the scene
into `examples/coral_list.ts` by hand; the note says "the compiler does not yet choose". The compiler has no cost model, and the only measurements are from one Apple M4 shared with other workers, with a
0.065 ms timer tick; no p50 or p95 on any target GPU exists. Level of detail is planned on the CPU from bounds read
back from the GPU. Grids are non-indexed, so each grid point's position function runs about six times.

*Why.* Each schedule was built as a drawable with its own emitter before there was one place that could compare
them.

*Throw out.* Method as an API name. Unmeasured choice. The CPU in the level-of-detail loop.

### 5.7 G7 is partial: the scene is short and the language is not

A first scene is about twenty lines. Then: 816 public names in one namespace; `loft context` is 2,819 words today
(measured) and `docs/api.md` is 45,000; arithmetic is prefix calls
(`mul(belly, sub(1, mul(sub(1, rib), 0.05)))`) unless the author switches to the `wgsl` template, a second
authoring language with its own rules about what stays analysable; examples need casts and `lift<"f32">(1)`. A new *estimator* or a new *frame* has no path short
of writing a drawable against internal plumbing.

*Why.* TypeScript has no operator overloading, and the library grew by accretion around one subject (64% of example
lines are bosses).

*Throw out.* The prefix-call arithmetic as the main form (CORE already chooses WGSL text with holes). Subject
matter in the core API. Two ways to write one formula.

### 5.8 G8 is partial: open at the edges, welded in the middle

A style is a replaceable value, raw WGSL is allowed, and the host is replaceable. But a scene must have a ground, a
sun above the horizon and a style with eleven required members; every lit fragment calls `sea_height`; phases are
named `on-bodies`, `water`, `on-water`; the sky, ground and a shadow drawable are prepended to every scene. Truth
covers the CPU evaluation of the graph, not the picture: no reference to compare a frame against, no check that a
material conserves energy. And there are two engines: the Rust path builds 232 of 295 corpus entries and draws none.

*Throw out.* Mandatory content. The second engine, whichever it is, at the cutover.

## 6. What to throw out, in one list

From prior art: R1 to R7 as stated in section 2, plus the CPU scene graph and per-object submission in the frame
loop, the mandatory cook, the global `map(p)` with finite-difference normals and hack shadows, design to the weakest
device, and any new shading language.

From Loft:

1. A shader module per thing; shaders by string concatenation; inlining at record time.
2. Light, shadow, sky, ground and water as members of a style; PBR as "a style".
3. The display-referred 8-bit frame, tone mapping in the material, mandatory MSAA.
4. "Nothing carries between frames" and byte-identical pixels as engine invariants; headroom.md's "never" list.
5. The cap of seven lights; lights as distinct code; capsule-only shadows and rays.
6. Content in the core (sea, sun, ground, ink, layer, style) and the `paint` and `hit` signatures that encode it;
    closed lists of ways to cover pixels; "nothing back" from the host.
7. Method chosen by API name; choices made without a measurement.
8. Prefix-call arithmetic as the primary form; 816 names; subject matter in the engine's namespace.
9. Two engines.

## 7. What step 2 has to answer

Five questions, stated without answers.

1. **What one question does every kind of object answer, so that light transport is written once?** Teardown got
   uniform rays by making everything voxels; Unreal keeps four proxies. What is the smallest interface (a ray query,
   a bound, a surface point with material coordinates, perhaps a sampling rule) that an analytic primitive, a field,
   a triangle set, a medium and a particle set each implement in a few lines, and that a shadow, a reflection, a
   reservoir and a path are all written against?
2. **What is the core, if the ten types are library?**
3. **How does a composition compile without permutations**, with module count following kinds of code, and
   specialise against dispatch decided per site by cost?
4. **Where do history and noise live**, so that real-time reuse and the frame-alone reference are two schedules of
   one estimator and the difference between them is a reported number?
5. **What is the cost model**, in what units, on which GPUs, that lets the compiler choose analytic, marched,
   rasterised or traced per object and be scored for it?

## 8. What I did not verify

- Every **[recalled]** number. Those that would change a decision if wrong: ReSTIR's cost being independent of light
  count (I am confident of it), and the WebGPU limits against the wgpu version Loft pins.
- I ran no renderer and measured nothing except the word count of `loft context`. Loft numbers are from the
  repository's notes and the same day's third-party review.
- I read headers and the Rust plan's type sections, not every file. The `paint` and `hit` signatures are from
  `rust/crates/loft-core/src/draw/mod.rs` lines 1 to 20 and plan section 1.5.
- The scoring table in section 1 is judgment, not measurement.
