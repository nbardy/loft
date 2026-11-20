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
