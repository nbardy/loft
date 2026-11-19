# Render and light, step 2: what we add

Fable, 19 Nov 2026. Step 2 of three (critique, **invent and architect**, implement). Step 1 is
`render_fable_step1.md`. Its decisions D1 to D11, tests T1 to T8 and eight questions are used here by number.

**What I took the request to be.** Say what Loft adds so that render and light are right: the code a consumer
writes, the pieces under it, what is new, what it costs to the number, the types, and a plan from the tree at
`80b2d8e`. One file. Nothing was built, and nothing was run but a tokenizer.

**What I read.** The three pages and step 1. In `rust/crates`: `loft-ir`, `loft-core` (with its 13 WGSL templates)
and `loft-gpu`'s binding contract. `src/record` and `src/text`. The lean Rust plan, sections 1.5 and 8;
`headroom.md` section 7; `tracing_sota.md`; the third-party audit's measurements. I did not read the other step 2
drafts in this folder.

**How numbers are marked.** `[repo]` measured in this repository. `[derived]` arithmetic on the numbers beside it.
`[conjecture]` a constant I chose, with a band. Each is measured before the phase that depends on it is scored.
Token counts are `o200k_base` over the exact text shown.

## 0. The design on one page

Each Loft drawable answers "how is this thing drawn". That question hides three, and a different party should own
each.

1. **What is it?** A **kind**: a WGSL struct and the functions written for it. A shape kind says how a ray meets it
   (`hit`), how far a point is from it (`dist`) or where a parameter lands (`place`). A material kind says how it
   scatters (`eval`). Consumers write kinds. Instances are rows of a table.
2. **Who is seen, and what light arrives?** Six **queries** over the whole scene: `trace`, `visible`, `material`,
   `light`, `sky`, `medium`. Each is library code over a dispatcher that the compiler writes from the kinds present.
   An estimator is any function that calls them.
3. **When, how often and by what method?** A **schedule**: the pass, the rate (per sample, pixel, vertex, body
   texel, frame, load or build), the method (raster, exact ray, march, lookup) and the compile tier. The compiler and
   the engine choose it from a cost model. The consumer overrides it in code. The laws do not change when it changes.

An **engine** is a list of passes. Loft ships three as examples, each written only with public pieces: `reference`
(a path tracer, 24 lines in section 1), `realtime`, and `print` (toon and woodblock). They call the same queries over
the same kinds. So a new shape or material appears in all three, in shadows, bounces and the reference, with no
further code.

Five decisions carry the design.

- **A. Kinds over a visibility record.** A pixel first records who it sees, then one pass shades it. GPU code grows
  with distinct laws and never with instances.
- **B. One estimator, two schedules.** The real-time picture is the reference estimator under schedule statements
  (fewer bounces, fewer samples, slower rates). Each statement is scored against the reference.
- **C. Rates reach the body and time.** A term can be kept on a body's own chart. History is a stepped table that a
  cold frame replays.
- **D. Tiers by lifetime.** Interpret, share or specialise by a rule in the number of times a program will ever run.
- **E. The core forgets rendering.** It knows functions, tables, kinds, passes and rates. `ray`, `hit`, `medium`,
  `ground`, `outline` and `sun` leave Rust and become library text.

## 1. The consumer's code first

Written before the design and read cold. `k` is a `wgsl` module. `things`, `camera` and `SUN` in later cases are
those of case 1.

### Case 1. A scene: five methods in one frame (525 tokens)

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

The plane and the spheres are intersected exactly. The knot is sphere-traced inside its bound. The vase is
rasterised as a grid, and rays meet it through its patch tree. The fog is integrated in closed form. Nothing names
a method. `--engine reference` draws the same file with the path tracer.

### Case 2. A new primitive (421 tokens)

```ts
const k = wgsl`
struct Bead { R: f32, bore: f32 }
fn bound(s: Bead) -> Box { return Box(vec3f(-s.R), vec3f(s.R)); }
fn hit(s: Bead, ray: Ray) -> Hit {
  let o = ray.o; let d = ray.d;
  let b = dot(o, d);
  let h = b * b - dot(o, o) + s.R * s.R;                        // ball: t² + 2bt + c = 0
  let a = max(dot(d.xz, d.xz), 1e-9); let e = dot(o.xz, d.xz);
  let g = e * e - a * (dot(o.xz, o.xz) - s.bore * s.bore);      // bore: a cylinder about y
  if (h < 0) { return miss; }
  let s0 = -b - sqrt(h); let s1 = -b + sqrt(h);
  let c0 = (-e - sqrt(max(g, 0))) / a; let c1 = (-e + sqrt(max(g, 0))) / a;
  let inside = g >= 0 && s0 >= c0 && s0 < c1;                   // the ball's entry falls in the bore
  let t = select(s0, c1, inside);                               // then the first surface is the bore's far wall
  if (t < ray.t0 || t >= min(s1, ray.t1)) { return miss; }
  let p = o + t * d;
  return Hit(t, select(normalize(p), normalize(vec3f(-p.x, 0, -p.z)), inside), p.xy);
}`;

thing(k.Bead(1, 0.35), metal([1, 0.8, 0.5], 0.2), at([0, 1, 0]))
```

The same solid is `examples/bead.ts` today: 979 tokens as a file, in call form `[repo]`. This kind is found by the
camera, casts and receives shadows against every other kind, is met by bounce rays and by the reference, and has a
CPU twin for checks. None of that is written here.

### Case 3. A new BRDF (231 tokens)

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

`eval` alone is enough. The library derives `sample` (cosine-weighted) and `albedo`. Writing `sample` makes the
picture converge faster and changes nothing else. At build the CPU twin integrates `eval` over the hemisphere and
reports whether the lobe conserves energy.

### Case 4. A new estimator: a toon look with true shadows (165 tokens)

```ts
const k = wgsl`
fn shade(x: Point, wo: vec3f) -> vec3f {                       // radiance leaving x toward wo
  let m = material(x);
  let s = light(0, x.p);                                       // the sun
  let lit = visible(x.p, s.p).r * max(dot(x.n, s.wi), 0);      // true shadows, from every shape and medium
  let tone = 0.25 + 0.35 * step(0.05, lit) + 0.4 * step(0.5, lit);
  return albedo(m) * s.L * tone;
}`;

export default scene({ things, sky: sky(SUN), camera, engine: realtime({ shade: k.shade }) });
```

A look is an estimator. This one keeps the default engine's find pass, shadow query and materials, and replaces
only what light does at a point.

### Case 5. A whole engine: the reference path tracer (356 tokens)

```ts
const sum = target("rgba32float");
const k = wgsl`
fn path(px: vec2f) -> vec4f {                                  // one path through pixel px
  var ray = camera_ray(px + vec2f(rand(), rand()));
  var L = vec3f(0); var T = vec3f(1); var sharp = true;
  for (var bounce = 0; bounce < 16; bounce++) {
    let x = trace(ray);
    if (x.t < 0) { L += T * sky(ray.d); break; }
    let m = material(x);
    if (sharp) { L += T * emit(m, -ray.d); }                   // a light met by chance counts only where none was sampled
    let l = light(pick(), x.p);
    L += T * eval(m, -ray.d, l.wi) * l.L * visible(x.p, l.p) / l.pdf;
    let s = sample(m, -ray.d);
    T *= s.weight; sharp = s.pdf == 0;                         // pdf 0 marks a mirror-like lobe
    ray = Ray(x.p, s.wi, 1e-4, 1e9);
  }
  return vec4f(L, 1);
}
fn resolve(px: vec2f) -> vec4f { let s = load(${sum}, px); return vec4f(tone(s.rgb / s.a), 1); }`;

export const reference = () => [
  screen(k.path, { into: sum, blend: "add" }),                 // every run adds one sample per pixel
  screen(k.resolve, { into: view }),
];
```

This is the judge of T3. It is 24 lines because it uses every contract and schedules nothing. The shipped one adds
media by delta tracking and multiple importance sampling, in about 60 lines.

### Case 6. The default real-time estimator, and the one schedule word (148 tokens)

```ts
const E = cached(k.gather, { texels: 16, blend: 0.25 });       // light arriving, kept on each body's own chart
const d = wgsl`
fn shade(x: Point, wo: vec3f) -> vec3f {
  let m = material(x);
  var L = emit(m, wo) + albedo(m) * ${E}(x) / PI;
  for (var i = 0u; i < lights; i++) {
    let l = light(i, x.p);
    L += eval(m, wo, l.wi) * l.L * visible(x.p, l.p) / l.pdf;
  }
  return L;
}`;
```

`gather` is one cosine-weighted `trace` from a point, which reads this same cache where it lands. `cached` runs it
per texel of each body's chart, not per pixel. Deleting the word gives the same picture at pixel rate, slower.

### What a stranger asked, and what changed

- *"What is the 0.7 in `field(k.knot, 0.7)`?"* The bound is now a shape: `field(k.knot, sphere(0.7))`.
- *"Is fog a thing with no material?"* A thing is a shape, a response and a pose. A surface response covers the
  shape. A medium response fills it.
- *"Where are the lights?"* Anything whose response glows, and the sky. There is no light list and no cap.
- *"`surface` is a shape and a query?"* The query became `material`.
- *"What must `uv` be?"* The shape's own chart: any two numbers that name the point. Materials and caches index by it.
- *"What do I import for `Ray`, `trace`, `rand`?"* Nothing. They are the interface text every `wgsl` module is read
  with (section 5).

## 2. The architecture

```text
engines            reference · realtime · print            pass lists; examples; any size
sub meta engines   render · light                          small interfaces; library written to be rewritten
core               function · table · kind · pass · rate   tiny; knows nothing of rays or light
                   compiler, runtime, CPU twin, checks, solve
```

| | Interfaces | Orchestration | Library |
|---|---|---|---|
| render | The shape readings, `Ray`, `Hit`, the visibility record | Covers, `find`, `screen`, adapters | Shapes, `march`, `bricks`, `patches`, post passes |
| light | The six queries, the surface, medium and light readings, `cached` | `accel`, the emitter table, history replay, in-scatter | BSDFs, lights, skies, media, estimators, the three engines |

The interfaces are 6 structs, 12 reading names and 6 queries (section 5).

### 2.1 The core: five nouns

| Noun | What it is | In the tree today |
|---|---|---|
| function | WGSL text with holes, lowered to the graph. A call stays a call | Yes. Emit already shares by hash of the text |
| table | Rows or a texture, with a fill law and a clock (build, load, frame, steps) | Yes: kernels, packs, `Keep` |
| kind | A struct, the table of its instances, and its functions. An **open function** is a name that several kinds define, called with a kind chosen by data | **New** |
| pass | One run of a GPU program: compute over a grid, or raster draws into targets, reading tables and targets | Fixed forms only (`Cover`, `Compose`). Generalised |
| rate | The set of binders a value reads: time, thing, texel, vertex, pixel, sample | Two bits, frame and item (`rate.rs`). Extended |

With these the core lifts every value to the slowest pass that binds what it reads, resolves open calls, chooses
tiers from the cost model, emits one module per pass, lays out tables, and runs kernels and passes (2.5). It offers
the twin, intervals, gradients, checks and solve to any function in the subset.

**Open functions.** WGSL has no function pointers, so a choice among laws is a `switch`. The compiler
writes it, and only it. An open call whose kind is known at compile time is a direct call. Otherwise it is a call
to a generated function whose cases are the kinds present in this composition. A value of an open type, such as
`Bsdf`, is a kind id and a payload as wide as the widest kind present. With one kind present it is that struct and
there is no switch. WGSL has no overloads, so emitted names carry their kind (`Shell_eval`). The dispatchers are the
only GPU code no author wrote, and `loft show` prints them.

**Holes are fields.** A number in a hole or a constructor becomes a value in the kind's row, never text. Two things
that differ in constants share all their code. This is the rule that enforces D2's discard, and it replaces the
text scanner in `src/host/share.ts`.

### 2.2 Render: readings, covers and the visibility record

A shape kind gives at least one reading. Library adapters derive the others. The analyses make each derivation
true, or name it as unknown.

| Has | Wants | Adapter (library code) | Made true by |
|---|---|---|---|
| `dist` | `hit` | `march`: sphere trace inside the bound, normal from the symbolic gradient | Intervals bound the slope. An unbounded slope is `Unproved` by name |
| `dist`, dear | `hit` | `bricks`: bake to bricks in body coordinates at load, march them, refine on the formula | The cost model |
| `place` | pixels | The `grids` cover | — |
| `place` | `hit` | `patches`: the grid's quadtree of boxes, refitted each frame by table fills, two triangles a leaf | Exact for the triangles drawn |
| triangles | `hit` | A hierarchy built once at load in body coordinates | — |
| `hit` | pixels | The `boxes` cover, from `bound` | The twin samples rays: no hit lies outside the bound |
| `eval` | `sample` | Cosine-weighted | The twin: weight equals `eval` over pdf |
| `eval` | `albedo` | `π · eval(z, z)`, exact for a matte lobe | Declared approximate otherwise |
| `sigma` | `tau` | March | Step from the slope bound |

A reading that is missing with no adapter is a refusal. It names the reading and the adapter that would supply it.

**The visibility record.** A pixel stores two `u32`, `thing` and `where`, and a depth: 12 bytes. The
cover that found the pixel writes `where`: packed (u, v) for a grid, a triangle index for a mesh, nothing for a
shape found by a ray. The shading pass derives the surface again from the record. For a ray-found shape it calls
`hit` on the camera ray clipped to the recorded depth. For a grid it evaluates `place` and its symbolic gradient at
the recorded (u, v), which gives an exact normal and no interpolation.

**Covers** are the library's five ways to offer candidate pixels: `boxes`, `grids`, `meshes`, `quads` and `screen`.
They are today's 13 templates, reorganised. A cover is one pipeline. Kinds dispatch inside it by instance, so the
choice is uniform over each instance and costs no divergence. The rasteriser only bins candidates per pixel, and
its fragment stage runs only the intersector. Materials and light run once per pixel, not once per overlapping
proxy (D1).

Finding from a light's view, with the same covers and kinds, is a shadow map. It is a declared approximation of
`visible`, limited by resolution and scored against the reference. It is not a second scene (D5).

### 2.3 Light: six queries, estimators and declared history

| Query | Answers | Built from |
|---|---|---|
| `trace(ray) -> Point` | The nearest surface | `hit` of every shape kind, through `accel` |
| `visible(a, b) -> vec3f` | Transmittance from a to b: zero behind a surface, else the product over media | `hit` and `tau`. This is render.md's "one transmittance" (D9) |
| `material(x) -> Bsdf` | The response at x. `eval`, `sample`, `emit` and `albedo` apply to it | Each thing's response |
| `light(i, p) -> Lit` | A sample of light i toward p. `lights` counts them. `pick()` draws one by power | Light kinds, and glowing things by area |
| `sky(d) -> vec3f` | Radiance from direction d | The scene's sky, a closed form |
| `medium(p) -> vec4f` | Scattering and extinction at p, summed over media | `sigma` |

These six are the whole seam between render and light. A path tracer and a print style are both served, because
`light` and `visible` are separate calls.

`accel` is a library value. `flat` loops over things and serves up to about 30. `groups` uses the authoring
hierarchy (a body and its parts) as the hierarchy and refits bounds each frame with one table fill per level and no
sort. A sorted hierarchy and a hardware ray query are later backends for the mesh kind, behind the same `hit`.

Transparent surfaces are BSDFs that transmit, met by rays. There is no sorted alpha. One in-scatter pass marches the
camera ray once through the sum of all media, so media that interpenetrate are right by construction.

**History is declared.** There are two forms, and both are tables with a clock.

- *On the body.* `cached(f, { texels, blend, window })` is a stepped table on each body's chart. The key is the body
  point, so nothing is reprojected and nothing ghosts when the camera moves. A cold frame at time t replays the
  cache's `window` steps from t − window·dt. Time is an argument, so those steps can be drawn from nothing, and
  they cost body-rate work, not pixel-rate work. The cold frame and the warm frame differ by at most
  (1 − blend)^window of the cached quantity's range, and the identity record states that bound.
- *On the screen*, for the viewer only. A view-sized table accumulates samples. It is keyed by the visibility
  record: the same body point's pixel at t − dt is found by evaluating its pose and `place` at t − dt, which is
  exact for anything animated by a formula. Reuse is accepted where the record matches, not where colours look alike.

Clips and stills take their samples inside the frame and use no screen history. So any frame of a clip can be drawn
alone and matches.

### 2.4 The three engines

```ts
export const realtime = (o = {}) => (s: Scene) => [
  ...s.lights.filter(mapped).map((l) => find(s, { from: l.view, into: l.map })),   // a shadow map is `find` from a light
  find(s, { into: vis }),                                    // one draw per cover; kinds dispatch inside
  screen(shadePixel(s, o.shade ?? direct), { into: radiance }),
  screen(inscatter(s), { into: radiance, blend: "over" }),
  ...(o.post ?? [bloom, tonemap]).map((p) => p(radiance, view)),
];
```

`radiance` is linear and scene-referred until the last pass (D8). `print` is `realtime` with a toon or woodblock
`shade` and an ink pass over edges of the visibility record. `reference` is case 5. Every piece named here is an
ordinary export a consumer can copy.

### 2.5 How a composition compiles

1. **Record.** TypeScript records functions, structs, tables, things and the engine's pass list into typed arrays.
2. **Close.** The compiler collects the kinds reachable from each pass, derives missing readings, and refuses by
   name what cannot be derived.
3. **Lift.** Every value moves to the slowest pass that binds what it reads: build (the tape), load, frame, thing,
   texel, vertex, pixel, sample. Today's frame frontier (`rate.rs`) is this pass with two binders.
4. **Plan.** The cost model picks a cover per kind, an adapter per derived reading, a rate where the author wrote
   `cached`, a tier per program, and inline or call per site. The plan is a table of sites with the predicted time
   of each choice. `loft show --plan` prints it. A consumer pins a choice by passing a function, not a flag.
5. **Emit.** One module per pass holds the laws reachable from its entry and their dispatchers. Laws stay
   functions, so the printed module reads like the authored text and is thousands of lines, not a million.
   Every line keeps its author site, as `emit` does now.
6. **Lay out.** Things, poses, bounds, kind rows and caches go into packs. A module's text depends on the set of
   laws only. Kind ids come from content hashes, so the pipeline cache key is stable under any change of instances,
   constants or counts.
7. **Tier.** A program that will run fewer than about 5·10⁵ times in its life runs on the GPU tape interpreter, a
   fixed kernel whose pipeline is always cached. Others compile once, shared (4.6).
8. **Run.** The runtime writes the frame uniform, runs kernels, steps stepped tables, and runs the passes in order
   on a device the caller may own.

### 2.6 Replacing any part

| To change | Write | It plugs in because |
|---|---|---|
| A value | A struct value or a function of `Point` | Instances are rows |
| A shape, BSDF, light or medium | A struct and a reading (cases 2, 3) | Open functions |
| An adapter (`march`, `patches`) | A function from a kind to WGSL text | Adapters are library values the engine lists |
| What light does at a point | `shade(x, wo)` (case 4) | The six queries |
| How rays find things | `trace` and `visible` over the `hit` dispatcher | `accel` is an argument |
| Where a term is computed | `cached`, or remove it (case 6) | Rates |
| A pass, or their order | Copy the engine's few lines and edit | An engine is a list |
| The engine | A pass list and its laws (case 5) | The core runs any list |
| A scheduling decision | A function passed to the engine | The plan is data |

Nothing in the right-hand column belongs to a built-in.

## 3. What we must invent

Each piece below has an ancestor, named in section 6. The claim is the combination, and I know no engine that has it.

**I1. Consumer laws with a content-independent program count.** id Tech has few programs because content defines
no laws. Engines that let content define laws multiply programs (D2). Kinds, open functions and the visibility
record give both at once: the module count is a property of the engine, and lines follow distinct laws.

**I2. The reference and the real-time picture are one estimator.** Falcor shares material code between the two.
Here the estimator is shared too. An approximation is a schedule statement (a bounce limit, a sample count,
`cached`, a shadow map) that can be removed one at a time. So Loft can report which statement costs which error.

**I3. A body rate.** Every Loft shape has a chart by construction, so "per texel of the body" is a rate the
compiler can lift to, like "per frame". Light gathered there costs rays per texel, not per pixel. It does not
ghost, because the key is the point itself. It does not leak through a wall, because the key is a surface point
and its side.

**I4. History that can be replayed.** Time is an argument, so a cold frame recomputes its history. A frame alone
and temporal reuse are then compatible, and the difference has a stated bound that a check can measure. No engine
with hidden history can offer this.

**I5. Tiers chosen by lifetime, with an interpreter for author laws.** Dolphin and Godot fall back to a shared
shader while a specialised one compiles. Loft adds a tier below (the tape, on the GPU) and a rule in the number of
runs, because a model's scene may be drawn sixteen times and never again.

**Supporting pieces:** truth for light (a `litBy` check goes through `visible`; a consumer's BSDF gets furnace and
sampling verdicts from the twin), and a printed plan with predicted costs.

Where each aims past the state of the art:

| Aim | Best known | Loft target | Scored in |
|---|---|---|---|
| Load of a scene with new laws | Loft: 0.4 to 54.9 s `[repo]`. Engines: a cook, or stutter | 0.23 s small, 1.45 s for five crabs, the same for one crab | Phase 1, Phase 2 |
| A frame that mixes every method under one transport | No engine passes T1 and T2 together | Five crabs with true shadows and one bounce: 5.3 ms at 1280 × 800 | Phase 3 |
| A training frame, 512² | 4.26 ms with no transport `[repo]` | 16 samples with true shadows and one bounce in 23 ms | Phase 1 |
| A new primitive in shadows, bounces and the reference | An engine fork (Unreal); 342 to 447 lines (Bevy) `[step 1]` | 421 tokens | Case 2 |
| A new estimator | Not possible outside research frameworks | 165 tokens | Case 4 |
| Real time judged by its own reference | Falcor, for materials | Every schedule statement scored | Phase 1 |

## 4. Performance theory, to the number

All figures are for the M4 that every `[repo]` number came from.

### 4.1 Constants

| | Value | Band | Source |
|---|---|---|---|
| c, one scalar operation at pixel rate | 2 ps | 1 to 4 | `[conjecture]`, consistent with the coral's 0.8 ms per formula evaluation over its bound `[repo: DESIGN.md 7]` |
| ℓ, one coherent table or texel load | 30 ps | 10 to 90 | "tens of G/s" `[repo: DESIGN.md 17.5]` |
| τb, one bound test (one load, ten operations) | 50 ps | | `[derived]` |
| τh, one analytic hit (three loads, sixty operations) | 0.2 ns | | `[derived]` |
| d, one pass or dispatch | 10 µs | 8.5 to 33 | `[repo: DESIGN.md 15]` |
| κ, one WGSL line through the driver | 60 µs | 13 to 74 | `[repo: audit table]` |
| b, one pipeline | 25 ms | 5 to 45 | `[conjecture]`. The lantern's 397 ms over 9 modules bounds it at 44 `[repo]` |
| ρ, interpreting against compiled | 30 | 10 to 60 | `[conjecture]` |

### 4.2 A ray

Over a flat list of n things with h real hits: n·τb + h·τh. With n = 30 and h = 2 that is 1.9 ns, or 0.53 G rays
a second `[derived]`. Over groups (g groups of m, e entered): (g + e·m)·τb + h·τh. For 658 things as 26 groups of 25
with two entered, 4.2 ns `[derived]`. A ray that enters a grid surface's patch tree pays about 5 ns more per part
entered, so about 14 ns in the five-crab scene, or 70 M rays a second `[conjecture]`. The headroom proposal guessed
90 to 240 M.

### 4.3 A frame

Per covered pixel, with everything at pixel rate and one bounce: derive the surface again 0.2, material 0.2, the
sun's `visible` 1.9, `eval` 0.3, the bounce's `trace` 1.9, half the bounces land (material, `visible`, `eval`) 1.2,
`sample` 0.2. Total 5.9 ns `[derived]`. With the bounce moved to the body cache it is 2.7 ns.

| Scene | find | shadows | cache | shade | media, post | Total |
|---|---|---|---|---|---|---|
| Slice: 30 analytic things, 1280 × 800, 1 sample, pixel rate | 0.5 | rays, in shade | — | 5.2 | 0.1 | **5.8 ms** |
| The same with `cached` (Phase 3) | 0.5 | rays | 0.1 | 2.4 | 0.1 | **3.1 ms** |
| The same at 512², 16 samples, pixel rate | 1.9 | rays | — | 20.8 | 0.1 | **23 ms** |
| Five crabs: 658 things, about 0.7 M triangles | 1.5 | 0.8, find from the sun | 1.2 | 1.3 | 0.5 | **5.3 ms** |

- Find, for the slice: 1.6 M fragments at τh plus 0.1 ns to write. For the crabs: today's draw stage is 3.47 ms
  with paint and 4× multisampling in the fragment `[repo]`. Without them, 1.5 ms `[conjecture]`.
- The shade pass over a flat list is **1.9 ms + 0.11 ms · N** at 1280 × 800 `[derived]`: 2.5 bound tests per pixel
  per thing. It gives 3.0 ms at N = 10 and 5.2 ms at N = 30. The flat list stops paying near 30 things.
- Crab cache: 658 things × 64 texels × 2 rays × 14 ns. Crab shade: 0.6 M pixels × 2.2 ns (`place` and its gradient,
  material, four map loads, `eval`, one cache lookup).
- Reference: about 6 rays and 2.5 shaded points per sample, 14 ns a pixel. That is 12 ms a sample at 1280 × 800: 64
  samples in 0.8 s and 1,024 in 13 s `[derived]`.

Today's five-crab frame is 4.26 ms `[repo]`. The target adds true shadows on everything, one bounce and a linear
frame for 1 ms more.

### 4.4 Load

load = a + b·P + κ·L, with P pipelines and L lines the driver receives.

| Scene | P | L | Predicted | Today `[repo]` |
|---|---|---|---|---|
| Slice, N = 10 to 1,000 | 6 | 1,400 | **0.23 s**, the same for every N | — |
| Five crabs | 22 | 15,000 `[conjecture]` | **1.45 s** | 54.9 s, 305 modules, 1,130,448 lines |
| One crab | 22 | 15,000 | 1.45 s | — |

P is bounded by the engine: five covers for the view and for shadow depth, shade, in-scatter, the cache kernel, four
post passes and one fill kernel per level. It does not count things.

Precompute at load, where it buys frame time. A 64³ brick bake of a 1,000-operation field is 0.5 ms compiled and
16 ms on the tape `[derived]`; today it is 1.0 to 2.7 s of build CPU `[repo: DESIGN.md 7]`. A cold frame's replay of
16 cache steps is about 24 ms for five crabs `[derived]`. A sorted hierarchy over 100,000 triangles is about 0.1 s
of CPU, once `[conjecture]`.

### 4.5 Memory

| | Bytes |
|---|---|
| Per pixel, realtime: visibility 8, depth 4, radiance 8, view 4 | 24 |
| With bloom mips and the viewer's screen history | 34.7: 35.5 MB at 1280 × 800, 9.1 MB at 512² |
| Per pixel, reference: sum 16, view 4 | 20 |
| Per thing: thing row, pose, bound, fields, seven vec4 | 112: 74 KB for 658 things, 11 MB for 100,000 |
| Body cache per thing: 8² or 16² texels of `rgba16float` | 512 or 2,048 |
| Shadow map, 2048² depth | 16.8 MB |

### 4.6 The tier rule

A site with L lines runs n operations per invocation, P times a frame, for F frames. Compiled it costs
κ·L + F·P·n·c. Interpreted it costs F·P·n·c·ρ. With two operations a line, interpreting wins when

  F·P < κ / (2·c·(ρ − 1)) = 60 µs / (2 × 2 ps × 29) ≈ **5 · 10⁵ runs in the site's life** `[derived]`.

- A load fill of 4,096 rows is interpreted, and saves a pipeline (25 ms).
- A frame fill of 2,000 rows in a 16-frame clip (32,000 runs) is interpreted. In the viewer it passes the line after
  260 frames, compiles in the background and swaps.
- A pixel-rate law in a 512² clip of 16 frames at 16 samples (6.7 · 10⁷ runs) is compiled.

Specialising a shared site saves Δn operations and costs a pipeline and ΔL lines. It pays when
F·P·Δn·c > b + κ·ΔL. For Δn = 10 and ΔL = 30 that is 1.3 · 10⁹ runs: about four minutes of a site that covers a
tenth of a 60 Hz frame. **So Loft never specialises for a clip, and the viewer does so only for a site that stays
hot for minutes.** Unconditional specialisation is where today's 54.9 s went (D3).

### 4.7 What would change the design

- κ above 150 µs a line at 15,000 lines (the driver's inliner going superlinear): split the shade module by
  response class with a tile mask.
- Find above twice its prediction because proxies write depth from the fragment stage: move dear kinds to a compute
  find seeded by the raster depth.
- A ray through grid surfaces above 30 ns: the sun stays on find-from-light and rays run at the body rate only.
- ρ above 100: drop the tape tier and keep one fused kernel module per level.
- Body-cache error above 5% of the indirect term outside one texel of a contact: gather at pixel rate with the
  screen history.

## 5. The data model and its types

The core's types, in `loft-ir`. They replace `Cover`, `RasterPrim`, `Find`, `Proxy`, `Medium`, `Integrate`,
`Compose`, `Target::Shadow` and `frame.sun`.

```rust
pub struct Kind   { name: NameId, fields: Box<[Field]>, rows: ResId }       // a struct; its instances are rows
pub struct Open   { name: NameId, sig: Sig, cases: Box<[(KindId, FnId)]> }  // an open function and who defines it
pub enum   Op     { /* today's, and */ Field(u16), Make(KindId), Open(OpenId) }
pub struct Pass   { run: Run, reads: Box<[ResId]>, writes: Box<[Write]>, repeat: u32 }
pub enum   Run    { Compute { entry: Entry, grid: [u32; 3] }, Raster { draws: Box<[Draw]> } }
pub struct Draw   { vertex: Entry, fragment: Entry, vertices: u32, instances: Count, state: Raster, layer: Layer }
pub enum   Count  { Fixed(u32), Rows(ResId), Indirect(ResId) }
pub struct Write  { target: ResId, blend: Blend, clear: Clear }
pub enum   Tier   { Tape, Shared, Special }
pub enum   Keep   { Latest, Every(u32), Window(u32) }                       // Window: a cold frame replays this many steps
pub struct Choice { site: Site, chosen: u16, predicted_ns: Box<[F32]> }     // one row of the printed plan
```

The interface text every `wgsl` module is read with. It is library text, and it prints.

```wgsl
struct Ray     { o: vec3f, d: vec3f, t0: f32, t1: f32 }
struct Hit     { t: f32, n: vec3f, uv: vec2f }             // a shape's answer in its own frame; t < 0 is a miss
struct Box     { lo: vec3f, hi: vec3f }
struct Point   { p: vec3f, n: vec3f, uv: vec2f, t: f32, thing: u32 }   // the scene's answer, in the world
struct Scatter { wi: vec3f, weight: vec3f, pdf: f32 }      // weight = f · cos / pdf
struct Lit     { wi: vec3f, L: vec3f, pdf: f32, p: vec3f } // a light's sample toward a point
```

The twelve reading names. A kind takes a role by defining one.

| Role | Reading | Signature |
|---|---|---|
| shape | `hit`, `bound` | `(s, Ray) -> Hit`, `(s) -> Box` |
| | `dist`, `place` | `(s, vec3f) -> f32`, `(s, vec2f) -> vec3f` |
| | `gap` | `(s, Ray) -> f32`: how near a missing ray passes. Step 1's kept contract; `print` reads it for ink and edge coverage |
| surface | `eval`, `sample` | `(m, wo, wi) -> vec3f`, `(m, wo) -> Scatter` |
| | `emit`, `albedo` | `(m, wo) -> vec3f`, `(m) -> vec3f` |
| medium | `sigma`, `tau` | `(m, vec3f) -> vec4f`, `(m, Ray) -> f32` |
| light | `light` | `(l, vec3f) -> Lit` |

Tables the render library declares. The core sees only rows.

| Table | A row | Clock |
|---|---|---|
| things | shape kind and row, response kind and row, pose row, group | build, or frame when a count moves |
| poses | position, scale, rotation: two vec4 | frame |
| bounds | a sphere per thing and per group | frame, one fill per level |
| kind rows | a kind's fields | build or frame, by rate |
| emitters | glowing things by power | frame |
| caches | an atlas of body charts | steps, `Keep::Window` |

Targets: `vis` (`rg32uint`), `depth`, `radiance` (`rgba16float`), `view`, and whatever an engine declares with
`target`.

Names a consumer learns: `scene`, `thing`, `many`, `at`, `look`, `cached`, `target`, `screen`, `find`, the three
engines, the six structs, the six queries and the twelve readings: 36 in all. Shapes, materials and lights are
library values with no API of their own. Today's entry point exports 451 runtime names `[repo]`.

## 6. Inherited and ours

| Inherited | From |
|---|---|
| The visibility buffer | Burns and Hunt 2013; Nanite |
| Shape, BSDF, light, medium, integrator; directions in the surface frame | pbrt |
| Interfaces resolved statically or by a generated switch, with a packed payload | Slang, Falcor |
| One material code under reference and real time | Falcor |
| Rates moved without editing the term | RenderMan; Spire 2016 |
| Shading on the object's own parameters | Reyes; texel shading; Lumen's surface cache |
| Algorithm apart from schedule, chosen by a cost model | Halide |
| A tape interpreted in a fixed kernel | Keeter 2020 |
| A shared shader while a special one compiles | Dolphin 2017; Godot 4.4 |
| Rasterise the bound, intersect inside | Teardown; Loft today |
| Resampled importance sampling; GGX with multiple-scattering compensation | Talbot, Bitterli; Kulla and Conty |
| Sampling tests for a BSDF | Mitsuba's chi-squared test |
| A scene hierarchy refitted and not rebuilt | Early ray tracers; `headroom.md` 7.2 for grids |

| Ours | Section |
|---|---|
| Consumer-written laws with a program count fixed by the engine: kinds, open functions and holes as fields over a visibility record of any `hit` | 2.1, 2.2, I1 |
| Readings derived by adapters and made true by intervals, gradients and the twin | 2.2 |
| The real-time engine as the reference estimator under removable schedule statements, each scored | 2.4, I2 |
| The body rate: a cache on the shape's own chart as a compiler rate | 2.3, I3 |
| History as a stepped table that a cold frame replays, with a stated bound | 2.3, I4 |
| Reuse keyed by identity, with exact reprojection from closed-form motion | 2.3 |
| A shadow map as `find` from a light, over the same kinds | 2.2 |
| The tier rule in lifetime runs, and the finding that specialisation does not pay for a clip | 4.6, I5 |
| Furnace and sampling verdicts for a consumer's BSDF at build; light checks through `visible` | 3 |
| A core with no rendering vocabulary | 2.1, E |

## 7. A phased plan from the current code

**Where the tree is** `[repo: CORE.md]`. The Rust path builds 232 of 295 corpus entries and draws none, because
the TS library records no draws yet. The port's next step would put `Draw`, `Cover`, `Find` and `Medium` on the
wire for 13 drawables. Nothing persisted depends on those decls. So this is the cheapest moment to change what a
draw is.

**Phase 0. Hold one step, build two instruments.** Do not put the draw-side decls on the wire. Keep everything
else in waves 0 to 2: the graph, the tape, intervals, checks, solve, kernels, packs, cameras and `loft-gpu`'s
loading. Build `tools/load_fit` (synthetic bundles with P in {1, 8, 32} and L in {100, 2,000, 20,000}, cache state
controlled, fitting a, b and κ) and `tools/calib` (kernels with a known operation and load count, giving c, ℓ and
d). A constant outside its band rewrites section 4 before Phase 1 is scored.

**Phase 1. The vertical slice.** On a branch, in Rust and a fixture in the manner of
`tests/fixtures/ball_bundle.ts`, not through the TS library.

- `loft-ir`: `Kind`, `Open`, the three ops, generic `Pass`, `Run` and `Draw`, the `rg32uint` and `rgba32float`
  formats. `loft-core`: one module per pass with its dispatchers, in place of `draw/mod.rs`. `loft-gpu`: run any
  pass list, with many targets, additive blend and `repeat`.
- Library: the interface text; `plane`, `sphere`, `capsule`, `box`; the `boxes` and `screen` covers; `flat` accel;
  `matte`, `metal` with multiple-scattering compensation, `glow`; a sun, a sky and area sampling of glowing
  spheres; the estimator of case 6 without `cached` and with one bounce; `reference`; `tonemap`.
- Ceilings: 900 lines of Rust added, 700 of WGSL, 300 of TS.
- Scene: N things in all (a plane, four capsules, two boxes, the bead of case 2, one glowing sphere, and spheres
  for the rest), a sun and a sky, at 1280 × 800.

| # | Check | Predicted | Passes if |
|---|---|---|---|
| 1 | Pipelines and module text for N = 10, 100, 1,000 | 6 pipelines, at most 1,500 lines | Module hashes are identical across N |
| 2 | Cold load after the device exists, each N | 0.23 s | At most 0.35 s, and a spread of at most 10% across N |
| 3 | The record from raster find against `trace` of the camera ray, two cameras | 0 pixels differ in `thing`; depth within 1e-4 relative | At most 0.01% of pixels differ |
| 4 | Furnace box: closed, albedo 0.5, every wall emits E. Radiance read inside | 1.75 E at two bounces, 2.00 E at sixteen (E(1 − ρ^(D+1))/(1 − ρ)) | Within 1% at 1,024 samples |
| 5 | White furnace: a metal ball at roughness 1 under a uniform sky of 0.5 | 0.50 at centre and rim. Step 1 predicted 0.16 and 0.38 for today's lobe | Within 2% |
| 6 | `realtime` against `reference` held to one bounce, 256 samples each | The same image in expectation: one `shade` function | Mean relative difference at most 1% |
| 7 | Shade pass GPU time, flat list, N = 10 and 30 | 3.0 and 5.2 ms: 1.9 ms + 0.11 ms · N | Slope and intercept within 30% once scaled by the measured c and ℓ; within a factor of two as stated |
| 8 | 512², 16 frames, N = 30, 16 samples a frame | 23 ms a frame | p95 at most 50 ms |
| 9 | The bead added with no other change | Two module texts change. The twin's `visible` agrees with the GPU's shadow term at 1,000 probe points | At least 99.5% agree, all misses within 1e-3 of a shadow edge |
| 10 | Tokens of cases 2 and 4 as built | 421 and 165 | Within 15% |
| 11 | `rg` in `loft-ir` and `loft-core` for ray, hit, medium, ground, outline, sun | 0 outside comments | 0 |

Stop rule: two failed fixes in one subsystem stop the phase, and the note says which prediction missed and why.

**Phase 2. Every shape, the corpus and the cutover.** The `grids`, `meshes` and `quads` covers; `march`, `bricks`
and `patches`; `groups`. The 13 drawables become kinds: strand, sheet and lathe give `place`; field and relief give
`dist`; impostors, traced and copies give `hit`; specks and glow are quads; density and volume are media. The sea is
a kind. `print` carries toon and woodblock. The gate changes with the design. Geometry and paint are compared with
an R0 drawn again under the `flat` look from `7f48109`, by the plan's picture rule. Light is judged against
`reference`, not against R0, whose shadows are stand-ins (D5). At cutover the TS engine, `draw/mod.rs` and the 13
templates are deleted. Predicted: five crabs load in 1.45 s (pass at 2 s) with 22 pipelines, the same count and text
as one crab. p95 frame at most 6 ms at 1280 × 800 with the sun found from its own view.

**Phase 3. Light.** `cached` and `Keep::Window`; screen history keyed by the record; `pick` and resampling for many
lights; the open `Bsdf` with clear coat, sheen, transmission, thin film, anisotropy and subsurface (a random walk in
`reference`, a declared fit in `realtime`); one in-scatter pass; exposure, bloom on a mip chain, depth of field and
motion blur by samples, grain and grade; checks through `visible`; furnace verdicts. Predicted: the slice at 3.1 ms
with `cached`; five crabs at 5.3 ms; a cold frame's replay of 16 steps under 25 ms; cold against warm inside
(0.75)^16 = 1.0% of range; the cached term within 5% of the reference outside one texel of a contact.

**Phase 4. Tiers and the plan.** The tape kernel on the GPU; background compile and swap; the planner with its
printed predictions and a calibration file per device; p95 in `loft profile`; level of detail chosen on the GPU with
indirect draws. Predicted: ρ = 30; a lantern-sized scene's first frame within 0.1 s of the device; planner
predictions within 30% of profile on the corpus median.

**Phase 5. Scale.** A sorted hierarchy for meshes, built at load and refitted; the hardware ray query behind the
mesh kind's `hit`; glTF in as kinds; reservoirs as declared state; Keel, where a subject is found and shaded
offscreen and composited into the game's pass with its depth.

## 8. Step 1's questions, answered

1. A frame alone, and reuse: history is a stepped table, replayed over a stated window (2.3).
2. Lines against specialisation: interpret under 5 · 10⁵ lifetime runs, share above, specialise almost never (4.6).
3. Rays without hardware: the `hit` dispatcher under a library `accel` (2.3, 4.2).
4. One material contract: an open `Bsdf`, with `light` and `visible` kept apart (2.3).
5. Primary visibility: two words and a depth (2.2).
6. Printable code: laws stay functions, one module a pass, a site a line (2.5).
7. Dispatch: generated switches, ids from content hashes, holes as fields (2.1).
8. Which engine first: `reference`, in Phase 1.

**For the owner to decide.** (1) Hold the port's draw-side wire for Phase 1, which costs the port a few days and
saves writing that wire twice. (2) Accept that light parity against R0 ends, and that the reference is the judge.
(3) Whether the viewer's default is one sample with screen history, or two samples and none.
