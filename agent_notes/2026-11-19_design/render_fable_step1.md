# Render, step 1: where engines fall short of Loft's goals

Fable, 19 Nov 2026. Step 1 of three: critique now, then invent and architect, then implement. Judged against
`docs/design/render.md`, `docs/PHILOSOPHY.md` and `CORE.md` only.

**What I took the request to be.** Say where today's renderers miss those goals, Loft included. For each, say what is
wrong, what history caused it, what it costs and what to discard. No new design. I read the three pages, Loft's source
and notes at `2276005`, and the evidence they name. I ran nothing on the GPU. I wrote this without reading the
earlier step-1 drafts in `high_effort_run/`, apart from a few lines of them that a grep for the 305-module figure
returned.

**How numbers are marked.** `[repo]` read or measured in this repository, file named. `[checked]` a published figure
confirmed against a source today. `[memory]` a published figure from memory, so approximate. `[derived]` arithmetic on
the numbers beside it.

## The finding

Every renderer I examined, Loft included, rests on the same short list of decisions, each made for a constraint that
is not ours: triangles as the only real geometry, the draw call as the owner of its pixels, GPU code that multiplies
with content, a material that is a fixed block of parameters or a finished colour, light transport assembled from
special cases, and state hidden in earlier frames. Each was a sound answer to a 1995 rasteriser, a 2006 material
editor, a 2013 console or a 2018 ray budget.

Loft's problem differs in four ways. Its scenes are closed-form functions of position and time in one graph the
engine can read. A model writes them new for every picture. A picture may be drawn for 16 frames and never again.
Time is an argument, so any frame can be recomputed from nothing. Under those facts most of the inherited decisions
cost more than they return. Two of them (code that multiplies with content, and one compile tier for everything)
directly cause Loft's worst measured number: 54.9 s to load a scene that then draws in 4.26 ms `[repo]`.

Sections: 1 the goals as tests, 2 eleven inherited decisions, 3 engine by engine, 4 Loft today, 5 what to discard
and keep, 6 questions for step 2, 7 predictions that score this critique.

## 1. The goals as eight tests

| | Test | From |
|---|---|---|
| T1 | One frame mixes analytic, field, triangle, traced, volume and particle parts, each exact, chosen per part | render.md, "What it draws" |
| T2 | All of them share one visibility, one light transport and one material law. A shadow or bounce ray meets the same scene the camera does | same |
| T3 | The real-time picture converges to a path-traced reference that runs the same laws | "Light and beauty" |
| T4 | Frame time, p50 and p95 | "Speed" |
| T5 | Load time and GPU code do not grow with content | "Speed" |
| T6 | Method and schedule are chosen per site by measured cost, with the author's code unchanged | "Speed" |
| T7 | A primitive, BRDF, light estimator or pass is a few lines and plugs into everything. No built-in is privileged. Generated code prints. Reports are true | "Developer experience"; PHILOSOPHY 1, 3 |
| T8 | Few tokens, a small core | PHILOSOPHY 0, 4 |

## 2. Eleven inherited decisions

### D1. The draw call owns its pixels

A drawable binds its own pipeline, and its fragment stage finds the surface, evaluates the material and loops the
lights in one program. Forward renderers do all of it there (three.js, Filament, Godot, Bevy's default, Loft).
Deferred ones (Unreal, HDRP) move the lights out and keep the rest.

- **Why.** The rasteriser interface of the 1990s: set state, then submit triangles.
- **Cost.** Overdraw multiplies the dearest code. In Loft a field's march and its paint run for every fragment of
  its proxy, and a fragment that writes depth loses early rejection. Standard WebGPU has no conservative-depth
  qualifier `[memory]`. Relief takes the serpent's frame from 4.7 to 8.2 ms, and the note names lost early depth as
  part of it `[repo: DESIGN.md 8.4]`. Cost goes by the draw, not the triangle: 15 to 30 µs a draw, 45 draws for one
  brig, 658 for five crabs `[repo: DESIGN.md 12.5; agent_notes/2026-11-19_audits/thirdparty_fable.md 4]`. Nothing
  outside the draw can ask what is at a point, so shadow rays, a second renderer and picking each need their own
  copy of the thing.
- **Discard.** A drawable that emits its own shaders. A thing should say what it is. The frame should decide how its
  pixels are found.

### D2. GPU code multiplies with content

Unreal compiles material × vertex factory × pass, and added PSO precaching in 5.2 after years of shipped stutter
`[checked]`. Unity multiplies keywords and strips variants at build. three.js bakes defines and light counts into
each material's program `[memory]`. Filament compiles each material's variants offline. Bevy specialises pipelines
by key on first use. Loft makes a module per drawn thing: 305 modules, 1,130,448 lines of WGSL and 54.9 s of load
for five crabs `[repo: thirdparty_fable.md 4]`.

- **Why.** Modularity was built as text expansion ahead of the shader compiler: a preprocessor, a node graph printed
  through a template, or Loft's recorder, which inlines every call (the Titan records 308k nodes `[repo: CORE.md]`).
  The GPU only ever sees whole expanded programs. id Tech escaped, with about 100 shaders and 350 pipeline states
  for DOOM 2016 `[memory]`, by not letting content define laws at all. No engine has both few programs and laws
  written by the consumer.
- **Cost.** Load grows with content, then cooks, caches and stutter follow. Loft's two large scenes loaded at 49
  and 74 µs per WGSL line (54,861 ms / 1,130,448 lines; 8,964 ms / 120,920 lines) `[derived; two points, cache
  state not controlled]`. At that rate a one-second load allows 14 to 20 thousand lines in total, however they are
  split into modules. For scenes a model writes fresh, a cache keyed on content never hits.
- **Discard.** Any path where an instance, a constant or a count changes GPU code. Also rediscovering sameness from
  text afterwards (`src/host/share.ts`).

### D3. One compile tier

Every engine compiles each program as if it will run for hours, and a cook or an install step pays for it. A
training clip is 16 frames at 512² `[repo: CORE.md]`, and a model's scene may be drawn once. JavaScript engines
learned to interpret first and compile what runs hot. Renderers have one tier.

- **Cost, in Loft's numbers.** A module costs 180 ms to load (54,861 / 305). The whole five-crab frame costs
  4.26 ms `[repo, derived]`. Over 16 frames a specialised module would have to save 11 ms a frame to repay itself,
  which is more than twice the frame it belongs to. The frame uses under 9% of a 50 ms budget, while the load is
  69 times the clip's whole draw budget of 0.8 s `[derived]`. The scarce resource is spent to save the abundant one.
- **Prior art, outside engines.** Keeter's MPR (2020) renders implicit surfaces by interpreting a tape in a fixed
  kernel, with no compile per shape `[memory]`. Loft's Rust twin is already such a tape (`loft-core/src/tape.rs`).
- **Discard.** Unconditional specialisation. "Specialise vs share" in render.md has to count compile time against
  the number of frames drawn.

### D4. Triangles are the only citizen

Nanite, hardware acceleration structures, shadow maps, G-buffers and glTF all assume triangles. A default three.js
sphere is 960 triangles `[derived from its 32 × 16 defaults]`. The exact answer is one quadratic per covered pixel.
Fields are guests: a full-screen quad outside the light, shadow and depth systems (three.js, Shadertoy), a mesh made
at import (Unreal), a cloud of splats (Dreams), or the slower "procedural" path of hardware ray tracing.

- **Why.** 1990s hardware drew triangles, tools exported them, and each later system was built to eat what existed.
- **Cost.** Unreal shows the end of the road. The author's shape becomes triangles, then Nanite clusters, then again
  a mesh distance field, a global distance field, surface-cache cards and a BVH: five lossy copies of one shape,
  which disagree with each other (D5), plus an offline build. No folds, no domain repetition, no exact analytic edge.
- **Discard.** The triangle as the scene's common denominator. The denominator the goals imply is anything a ray can
  be intersected with inside a proved bound. A mesh is one kind of it.

### D5. Secondary rays see another scene

Rasterising gives visibility from one projection. Every other question gets a stand-in. Lumen traces mesh distance
fields and cards: walls under about 10 cm leak, and skinned meshes are absent from its software scene `[memory]`.
Teardown traces a one-bit occupancy volume. DDGI asks a probe grid. Screen-space methods know only what the camera
saw. Shadow maps rasterise the scene again for each light.

Loft has three stand-ins that disagree. The sun's shadows are skeleton capsules projected on the ground only. A
lamp's 1024² depth picture leaves out impostors. `castRay` meets capsules. Soft `castRay` shadows cost +8.85 ms
against +1.31 ms for the map, and the two differ in 2.72% of pixels `[repo: DESIGN.md 17.7]`. "A thing without a
skeleton casts nothing" `[repo: docs/lighting.md]`.

- **Why.** No cheap ray query over triangle soup existed before 2018, and none exists in WebGPU now.
- **Cost.** Leaks, missing shadows and one tuning problem per stand-in. It also closes the road to T3: a picture
  built from stand-ins converges to the stand-ins.
- **Discard.** A stand-in used as a substitute. Two uses remain legitimate: an acceleration that the exact form then
  refines, and a declared approximation scored against the reference.

### D6. The material is a parameter block or a finished colour, and the engine owns the integral

G-buffers force an enumerated block: Unreal's shading-model id (4 bits `[memory]`), HDRP's material types,
Filament's five models `[checked]`, glTF's set in three.js. In Unreal a new shading model is an engine fork.

At the other pole Shadertoy, three.js `ShaderMaterial` and Loft return a finished colour: `style.shade(ctx)` in
TypeScript, and `paint(p, n, id) -> rgba` on the new Rust side `[repo: rust/crates/loft-core/src/draw/mod.rs]`. A
path tracer cannot sample a finished colour, and resampling cannot evaluate a target function for it.

The light loop is unrolled, so it is capped. Loft takes 7 lights and the sun at 0.05 ms each and refuses an eighth
`[repo: docs/lighting.md]`. Godot allows 512 clustered elements a view and 8 + 8 lights a mesh on mobile
`[checked]`. Filament allows 255 `[checked]`. A surface that emits lights nothing. Shadows, occlusion, reflections
and global illumination are separate subsystems, and Godot ships five for global illumination alone.

- **Why.** Fixed-function lighting, then ubershaders, then G-buffer bit budgets.
- **The exceptions.** pbrt's BxDF (`f`, `sample`, `pdf`), Falcor's `IBSDF` and RTXDI's application bridge make the
  material and the light into functions an estimator calls.
- **Cost.** T3 and half of T7. No estimator can be swapped, and no reference shares code with the real-time path.
- **Discard.** Both poles, light lists with caps, and shadow machinery per type of light.

### D7. History is hidden

Temporal antialiasing and upscaling, ReSTIR's reservoirs, DDGI's hysteresis (near 0.97 `[memory]`), Lumen's surface
cache and virtual shadow map pages all make the picture a function of frames the author never sees. No engine
declares that state, states its bias or bounds its lag.

- **Why.** Budgets of 1 to 8 rays a pixel. Without reuse, Cyberpunk's path tracing runs at 16 to 20 frames a second
  at native 4K on an RTX 4090 `[checked]`.
- **Cost.** Ghosting, boiling, and a frame that cannot be drawn alone. Loft's promise that a fresh process gives the
  same pixels at time t is broken by each of these as built. A reward computed on a cold frame differs from the
  warm one.
- **What they lack and Loft has.** Every state is a closed form of time. So history can be recomputed by drawing
  the frames before t, and the same body point at t − dt can be found exactly. Engines rasterise a velocity buffer
  and fail on anything animated inside a shader.
- **Discard.** Implicit history. Accumulation stays, as declared state with a law and a stated error.

### D8. The frame is fixed, opaque and display-referred

Unreal's pass order is engine C++. HDRP offers a short fixed list of injection points `[memory]`. Frostbite's frame
graph (2017) made the frame into data, which is right. Its passes are still opaque C++ callbacks, so it can cull a
pass and share memory but cannot fuse or specialise what it cannot read.

Loft has a fixed phase order, 4× MSAA into `rgba8unorm`, and the tone map inside each body's fragment
`[repo: src/host/manifest.ts, src/style/pbr.ts]`. Bloom and depth of field therefore read 8-bit display values.
Bloom is one pass of 40 taps because post cannot own a second target or a mip chain `[repo: src/style/post.ts]`. A
`draw.volume` writes linear in-scatter with premultiplied alpha over bodies that are already tone-mapped
`[repo: src/draw/volume.ts]`. Exposure does not exist.

- **Why.** Passes are imperative code in another language than the shaders, so no tool sees the whole frame. Loft's
  first style was a print, and a display-referred MSAA frame is right for a print.
- **Cost.** Highlights cannot bloom or defocus by their radiance, media composite in the wrong space, and nothing
  fuses across passes.
- **Discard.** Display-referred intermediates, pass lists the engine owns, and pass bodies the compiler cannot read.

### D9. Media and transparency are another renderer

A depth buffer holds one opaque depth and nothing about what lies in front of it. So engines add a froxel fog grid
(160 × 90 × 64 in Frostbite's 2015 version `[memory]`), a cloud system, a water system, particles that sample the
froxels, and transparent surfaces sorted by centre. Each has its own transmittance.

Loft has Gaussian `draw.density` (closed form, depth bins, a CPU light pass in O(N²) that reaches about 20 ms at 520
blobs), marched `draw.volume` (one transparent draw each, composited in author order, where "interpenetrating
transparents need per-ray interleaving"), a veil for specks implemented twice, and a `fog()` constant inside the PBR
style `[repo: DESIGN.md 11; src/style/pbr.ts]`. A cloud does not shadow a body. The storm costs 11 to 32 ms a frame
after tuning `[repo: DESIGN.md 11]`.

- **Cost.** Wrong order, missing shadows, and code per pair of kinds.
- **Discard.** Depth as the only quantity the methods share. render.md already names the replacement: one
  transmittance shared with surfaces.

### D10. The schedule is the author's code, and costs are folklore

Artists pick levels of detail, Nanite or not, baked or dynamic, and a shadow method per light. In Loft the author
picks `draw.surface`, `field`, `copies`, `traced` or `impostors`. `schedule: "auto"` exists for fields only, and it
decides by a node count of 1,000, not by GPU time `[repo: src/draw/field.ts:227]`.

- **Why.** Halide (2012) split algorithm from schedule. Spire (2016) moved shading terms between rates without
  editing them, and the idea did not survive into Slang, which chose compatibility with HLSL `[memory]`. Engines
  took neither, because vertex and fragment stages are entry points in the language itself.
- **Cost.** Tokens, since intent is restated per method. Speed, since a wrong choice stays. And no loop from
  measurement to choice. Loft's own instrument reports minima and medians on a shared machine
  `[repo: DESIGN.md 8.4, 11]`. The goal's statistic is p95, and the repository has none.
- **Discard.** Method names as the way to choose, and thresholds not derived from measured cost.

### D11. The escape hatch is a cliff, and the engine is too big to read

three.js `ShaderMaterial` leaves the lights, shadows and fog behind, and `onBeforeCompile` patches strings. Bevy's
own examples take 342 lines for a post-process pass, 424 for a custom phase item and 447 for a specialised mesh
pipeline `[checked]`. In Loft `draw.shader` is "unstyled, unseen by checks". A new light estimator cannot be written
at all, because the compiler owns the loop. The headroom proposal files a path tracer as "a second renderer, kept as
a separate tool" `[repo: docs/proposals/headroom.md 7.4]`. Loft is 47.9k lines of TypeScript and 15.9k of Rust after
nine days, with 451 runtime exports and about 350 types `[repo]`.

- **Why.** Features were added inside the engine, so each has access the consumer lacks.
- **Cost.** T7 and T8. A model cannot hold the engine in context, so every rewrite begins with archaeology.
- **Discard.** Privileged built-ins, and names that mirror internals.

## 3. Engine by engine

D-numbers refer to section 2. The table is my judgement, for orientation (`+` meets, `~` part, `-` fails). The
entries give the evidence. No row has T2, T4 and T7 together.

| | T1 | T2 | T3 | T4 | T5 | T6 | T7 | T8 |
|---|---|---|---|---|---|---|---|---|
| Unreal 5 | - | - | ~ | + | - | ~ | - | - |
| Unity HDRP | - | - | ~ | + | - | - | - | - |
| Godot 4 | - | - | - | + | ~ | - | ~ | ~ |
| Bevy | - | - | - | + | ~ | - | ~ | - |
| three.js | - | - | - | ~ | ~ | - | ~ | ~ |
| Filament | - | - | - | + | ~ | - | - | ~ |
| id Tech | - | - | ~ | + | + | - | - | - |
| Shadertoy | ~ | ~ | - | - | - | - | ~ | + |
| pbrt, Mitsuba | + | + | + | - | + | - | ~ | - |
| Falcor | ~ | + | + | ~ | - | - | ~ | - |
| Loft today | ~ | - | - | + | - | ~ | ~ | ~ |

**Unreal 5 (Nanite, Lumen).** *Wrong:* D2, D4, D5, D6, D7, D8. Nanite draws triangles only, and deformation,
translucency and masked materials came late or in part `[memory]`. Lumen needs four more copies of the scene.
*Why:* built for scanned film assets on 2020 consoles with weak ray tracing, on a material editor from 2006.
*Cost:* not frame time. Nanite culls and rasterises its demo in about 2.5 ms, 4.5 ms with materials
`[checked, second-hand]`. Lumen fits 4 ms at 60 Hz and 8 ms at 30 Hz at 1080p internal `[checked]`. The cost is
cooks, stutter, leaks and C++ that nobody outside Epic rewrites. *Discard:* the chain of copies, shaders per
material, undeclared history. *Keep:* the visibility buffer, which splits visibility from shading. Two-pass culling
against hierarchical depth. Screen-space radiance probes with importance-sampled gathers. A path tracer inside the
engine as the reference.

**Unity HDRP.** *Wrong:* D2, D6, D8, D9. Three pipelines with incompatible shaders. `Lit` is a tree of keyword
variants. Fog, clouds and water are separate systems. *Why:* ShaderLab keywords (2005) were the only modularity, and
the pipeline was split in 2018 to serve phones and consoles. *Cost:* a BRDF change forks `Lit`, and stripping
variants is a build discipline. *Discard:* keywords and pipeline forks. *Keep:* physical light units and exposure.

**Godot 4.** *Wrong:* D5, D6. Three renderers, five global illumination systems chosen by the user, and the caps
above. *Why:* a small team covering phones to desktops. *Cost:* capability. *Keep:* the `light()` function of its
shading language, a per-light law the user writes. It is the nearest thing in a mainstream engine to "consumers
define the laws".

**Bevy.** *Wrong:* D2, D11. Modularity sits in Rust traits and plugins and in `#import` and `#ifdef` over WGSL, not
in functions the engine can analyse. *Why:* ECS first, with a separate render world for parallelism. *Cost:* the
line counts in D11. Its meshlets and its ray-traced lighting need native-only features `[memory]`. *Keep:* wgpu and
naga as the single path. Also its meshlet result, cited in `docs/tracing_sota.md`: a compute rasteriser took the
first raster pass from 3.44 to 0.42 ms.

**three.js, Babylon.** *Wrong:* D1, D2, D4, D6, D11. A scene graph with work per object in JavaScript. A closed set
of materials. TSL, which writes maths as chained and nested calls. A separate path tracer that re-implements the
materials, so two material codes drift. *Why:* WebGL 1 had no compute, scene graphs came from Flash, and glTF is the
content model. *Cost:* a practical ceiling near a thousand draws on modest hardware `[memory]`. *Discard:* the
object graph as the runtime structure, and nested-call shading as the first authoring form (Loft found this itself
and moved to WGSL text). *Keep:* instances as data, glTF as the door for assets, and one graph with two backends.

**Filament.** *Wrong:* D2, D5, D6. Five fixed models with one per-light hook, forward only, materials compiled
offline by `matc`, 4 shadow cascades `[checked]`, no global illumination beyond image-based light and screen-space
terms. *Why:* Android phones, with tile GPUs, MSAA and no time to compile on the device. *Discard:* offline material
builds. *Keep:* its derivations (energy compensation, clear coat, cloth, sheen) as library maths.

**Frostbite.** *Wrong:* D2, D8. Its surfel global illumination is D5 and D7 again. *Why:* memory and feature
management across consoles. *Keep:* the frame as declared data, which Loft's manifest already is. Physical units.
Hillaire's sky and froxel media as laws for the library.

**id Tech.** *Wrong against these goals:* D4, D6. The fewest shaders in the industry, bought by forbidding author
laws. *Why:* a 60 Hz shooter whose engine team writes every shader. *Keep:* the number. A module count independent
of content is achievable, and frame time rewards it.

**The demoscene (Quilez, Shadertoy, 4k intros).** *Wrong:* one fragment shader. `map(p)` is an N-way `min` at every
step of every ray. Lighting is a hand-made rig per shader. Reuse is copy and paste. *Why:* 4 KB limits and a single
full-screen quad. *Cost:* pixels × steps × parts. Pruning the parts per cell is worth 8 to 227 times in the
published tables, and Loft's coral cost 11 to 14 ms marched `[repo: docs/tracing_sota.md]`. *Discard:* the global
union, unbounded rays, and lighting as a trick per scene. *Keep:* the algebra (intersectors, smooth unions, folds,
repetition) and its meaning per token, which is the bar for T8. One more point in its favour: its shadow rays march
the same `map` as its camera rays, so it passes the visibility half of T2 where Unreal fails.

**Dreams.** *Wrong:* fields are sampled into bricks and then into splats. The renderer is the look. Fields cannot
animate, only rigid pieces of them `[memory]`. *Why:* a PS4, sculpting with a controller, no UVs. *Keep:* edit lists
per brick, the ancestor of pruning.

**Teardown.** *Wrong:* D5. Camera rays march each object's own voxels, and secondary rays march a world-aligned
occupancy volume, so a rotated object's shadow is not its shape `[memory]`. *Why:* one developer, OpenGL,
destruction first. *Keep:* rasterise the bound, march inside and write depth, which Loft already does. Also one
cheap global structure for every secondary ray, used as an acceleration.

**ReSTIR, RTXDI.** *Wrong:* D6, D7. It needs fast shadow rays and a G-buffer block, its reservoirs are hidden
history, and the shipped path leans on closed denoisers. *Why:* 1 to 8 rays a pixel on RTX hardware, delivered as a
vendor kit. *Number:* 3.4 million emissive triangles in under 50 ms at 1080p with at most 8 rays a pixel, and 6 to
60 times faster at equal error for the unbiased form `[checked]`. *Keep:* resampled importance sampling as a library
law. It removes the light cap at a constant number of rays per pixel, which answers Loft's limit of 7.

**DDGI.** *Wrong:* D5, D7. A world grid placed by hand, 8 × 8 irradiance and 16 × 16 distance texels a probe
`[checked]`, diffuse only, leaks patched by heuristics. *Why:* a drop-in for engines that already had baked
irradiance volumes. *Keep:* octahedral storage and interpolation weighted by visibility.

**PBRT, Mitsuba.** *Wrong against these goals:* T4 and T7. Not real time. A new integrator is C++ and a rebuild.
Mitsuba's JIT needs LLVM or CUDA and seconds per kernel `[memory]`. Neither runs on a device a game owns. *Why:*
teaching and research at offline budgets. *Keep:* the decomposition into shape (`intersect`, bound), BxDF, light,
medium and integrator. pbrt already mixes analytic quadrics, curves, triangles and volumes under one transport. So
once rays are the currency, T1 and T2 are the natural design, and it was the rasteriser that broke them. Dr.Jit
shows that a whole composition can be traced and compiled as one kernel.

**Falcor, Slang.** *Wrong:* a pass is a C++ class, a Slang file and reflection, and it is built for DXR first.
Slang is a new language with little training text beside WGSL and GLSL, and it stops at the shader: no CPU twin, no
intervals, no rates, no passes. *Keep:* one material code under both the reference and the real-time passes, and
interfaces resolved statically or by a generated `switch`.

**Hardware ray tracing.** *Wrong:* D4 in silicon. The acceleration structure belongs to the driver, analytic shapes
take a slower path, moving meshes refit or rebuild every frame, and hit groups re-create D2. It is absent from
WebGPU and experimental in wgpu `[checked]`. *Keep:* inline ray queries as one backend for the mesh kind, behind
the same hit contract as compute.

**WebGPU and wgpu.** No ray tracing, mesh shaders or multi-draw indirect in the standard `[memory]`. By default 4
bind groups, 8 storage buffers and 16 sampled textures a stage `[checked]`. No function pointers or recursion in
WGSL. A pipeline cache on Vulkan only `[checked]`. A GPU clock that ticks in 0.065 ms here `[repo: AGENTS.md]`.
These are the only constraints worth accepting, and they point one way: few pipelines, few large tables, dispatch by
data, and compute where the fixed stages fall short.

## 4. Loft as it is today

In one respect Loft is ahead of most of the list. One frame already mixes exact impostors, sphere-traced fields
with bricks and pruned unions, relief, analytic surfaces evaluated in the vertex stage, meshes, two kinds of media
and particles, against one depth buffer. Scenes run 12 to 48 lines, and refusals name the fix. The steady frame is
cheap: 2.62 ms for the maelstrom and 4.26 ms for five crabs at 1280 × 800 on an M4 `[repo: thirdparty_fable.md 4]`.

| Test | State | Evidence `[repo]` |
|---|---|---|
| T1 | Part | No rays against an acceleration structure. No quadric, box or torus intersectors yet. The author chooses the method |
| T2 | Fails | Depth is shared. Transport and material are not. Three shadow stand-ins. In `pbr` the sun reaches every body unshadowed (`src/light.ts:208`). Occlusion exists for traced fields only, as four taps. An emitter lights nothing. Media sit outside (D9) |
| T3 | Fails | No reference. Still mode accumulates the same rasterised estimator, so it converges to the same missing terms. `tools/parity` compares against the old engine's own frames |
| T4 | Good when small | The storm is 11 to 32 ms, the coral 11 to 14 ms, depth of field 3.3 ms with up to 146 texture loads a pixel. Statistics are minima and medians, not p95 |
| T5 | Fails | 0.4 s to load a lantern, 9.0 s for the maelstrom, 54.9 s for five crabs. A field's brick bake takes 1.0 to 2.7 s of build CPU against a 2 s cap. About two dozen training programs miss that cap by 49 to 76 ms |
| T6 | Part | D10 |
| T7 | Part | Parts swap well: `pbr({ fresnel })`, a `hit` function, a style value. The estimator, the frame and the shape of a scene do not. A scene must have a ground, a sun and a style, and every lit surface shader calls `sea_height` (`src/draw/surface.ts:319`) |
| T8 | Part | Short scenes. About 800 exported names, and 2,200 words of `loft context` to learn them. Library maths in call form, such as `sub(mul(b, b), sub(dot(oc, oc), R * R))` in `examples/bead.ts` |

**The material gap, in numbers.** The default lobe reflects 0.307 of a uniform sky face-on at roughness 1, and
there is no multiple-scattering term `[repo: agent_notes/2026-11-18_pbr_style/NOTE.md]`. The white-furnace test
passes because its ball is a dielectric, whose diffuse term is weighted by one minus the lobe's albedo. A rough
white metal has no diffuse term and would return 0.31 of the sky face-on `[derived, not run]`. There is no clear
coat, sheen, transmission, subsurface or anisotropy. A highlight smaller than a pixel sparkles. The sky is three
colours and a glow, not an atmosphere. `litBy` evaluates the light's own function (`src/check.ts:370`), so as I
read it the check can pass on a body standing in that lamp's shadow, which conflicts with the truth rule in CORE.md.

**Why it happened.** Loft is nine days old and grew from one game's toon sea bosses. A print style wants a
display-referred MSAA frame, capsules are a fine shadow for a woodblock, and every scene then had a sea.
The recorder had expressions and no functions, so everything inlined and each thing became a module. A
byte-identical gate then kept every path alive in parallel. `pbr` arrived on day eight as a style inside that frame.

**What the lean Rust plan changes, and what it keeps.** It removes the parallel paths and folds five tracers into
one `Ray` template and four particle kinds into `Quads`. That is right, and 243 lines of WGSL templates show how
small the fixed part can be `[repo: rust/crates/loft-core/wgsl]`. It is still a port of the forward frame. As
written, each draw becomes one module (`draw/mod.rs:1`). `paint` returns rgba. `hit` is wired only into its own
proxy's fragment stage. There is no query over the scene, so a ray reaches another thing only when the author calls
that thing's function by hand. Light appears in the core only as a shadow-map target. D1, D5, D6, D8 and D9 cross
the port intact, and wave 3 will harden them into the wire unless step 2 says otherwise first. The parity gate is
correct for a port and wrong as a definition of light: the reference frames contain display-space compositing and
the energy loss above, so closing those gaps will read as a regression.

**Discard from Loft.**

- Modules per draw, the inlining recorder and the text `share` pass.
- The `Style` record with `water`, `ground`, `shadow` and `sky` members, and the required ground, sun and style.
- `paint -> rgba` as the material contract, on the Rust side too.
- The light list, its cap, and `sum | largest` as the only estimators.
- Capsule shadows, the lamp map and `castRay` as three separate systems.
- Tone mapping inside `shade`, the 8-bit view, and post on display values.
- Media composited one draw at a time in author order.
- Method names as the schedule.
- The sky projected into spherical harmonics as the only environment.
- Level of detail planned on the CPU from a GPU readback.
- Minima as the speed statistic.

**Keep. These are Loft's own.**

- One graph with a CPU twin, intervals and gradients over the author's own code.
- Time as an argument.
- Body coordinates, in which a shape does not move, so caches survive animation and reprojection can be exact.
- The hit contract `{ t, gap, normal }`. Its `gap` gives coverage and ink from the same function as the surface.
- The light fold that hands a style `unshadowed` and `visibility` separately. That is what lets a toon or a
  woodblock be a composition and not a second renderer.
- A closed-form sky used directly as an environment light, and closed-form Gaussian transmittance.
- Refusals that name the fix, and predictions written before the run.

## 5. Discard and keep, in one place

**Discard, from every engine.** The eleven decisions of section 2, each as its last line states, and with them
offline cooks as the place where speed comes from.

**Keep, inherited, by name.** The visibility buffer (Burns and Hunt 2013, Nanite). Culling against hierarchical
depth. Clustered culling. GGX with multiple-scattering compensation (Kulla and Conty 2017). pbrt's interfaces.
Resampled importance sampling (Talbot 2005, Bitterli 2020). Octahedral maps. Lipschitz pruning (Barbier and others
2025). Analytic intersectors (Quilez). The frame as data (O'Donnell 2017). Algorithm apart from schedule (Halide,
Spire). Trace, then compile (Dr.Jit). A tape interpreted in a fixed kernel (Keeter 2020). id's shader count.

## 6. What step 2 must resolve

1. **A frame alone, and temporal reuse.** render.md asks for both. D7 says what is wrong with reuse as built. The
   design has to say what the declared state is, how a cold frame obtains it, and what error it reports.
2. **Lines of GPU code against specialisation.** At 49 to 74 µs a line, what is the rule that decides between
   interpreting, sharing and specialising, as a function of frames to be drawn?
3. **Rays without hardware.** WebGPU has none, so a compute traversal over Loft's own forms must exist, and
   hardware is an optional backend. The headroom proposal guesses 90 to 240 million rays a second on the largest
   example `[repo, conjecture]`. Nobody has measured it.
4. **One material contract for two readers.** An estimator needs `eval`, `sample` and `pdf`. A print style needs
   arrivals with visibility kept apart. Both poles of D6 fail one of them.
5. **Primary visibility.** What a pixel records (who, where in body coordinates, at what t), and how a part found
   by a ray avoids the cost of writing depth from the fragment stage.
6. **Printable code and author-line errors** when the program is fused, shared or interpreted. A million printed
   lines answers nobody's question.
7. **Dispatch.** WGSL has no function pointers, so choosing among laws is a `switch` inside a module. Module
   identity is then the set of distinct laws, and the design must keep that set small and stable.
8. **Which engine is written first.** The path tracer is the judge in T3 and the simplest program that uses every
   contract. Written last, as a separate tool, it will inherit contracts that cannot serve it.

## 7. Predictions that score this critique

Each is stated before any run. A miss corrects the diagnosis, not the code.

1. **Load follows lines compiled (D2, D3).** Cold load of the five-crab scene is 49 to 74 µs times the WGSL lines
   the driver receives. If its distinct code fits in 20 thousand lines, cold load is at most 1.5 s whatever the
   number of crabs, and the steady frame stays within 20% of 4.26 ms.
2. **Energy (D6).** In the white-furnace set-up of `tools/pbr_sanity.ts` (uniform sky 0.5, no direct light, tone
   map off), a white metal ball at roughness 1 reads 0.16 at its centre and 0.38 near its rim. A conserving material
   reads 0.5 everywhere.
3. **Sun shadows (D5).** In `--style pbr` at a low sun, the crab's claw throws no shadow on its shell, while its
   capsule shadow lies on the water.
4. **Compositing space (D8, D9).** A medium that only absorbs, with transmittance 0.5, in front of a white surface
   of radiance 1 shows display value 0.45 under `pbr`. Composited in radiance and then toned it would show 0.80
   `[derived from the tone fit in src/style/pbr_parts.ts]`.
