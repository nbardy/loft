# Headroom: what the four proposals and the renderer under them still rule out

A design proposal and a review, 2026-11-13. Nothing here is built. It reads the three proposals now on main
(`materials.md`, `state.md`, `geometry_and_rays.md`) and the light sketch in `docs/open_design.md` (the light proposal
was not written when this was: where I need it, I work from the sketch and say so). Claims about the code were
checked at `29594fb`. Numbers marked *measured* were measured by the proposals I cite; numbers marked *estimate* are
mine, with the arithmetic shown, and are what a build will be scored against.

## 1. The verdict

**Does the set limit us?** For the looks Loft exists for, no: channels, a light as a function, author-written tracers
and baked state open the four largest doors, and none of them closes one another. For state-of-the-art light and
reflection, the four proposals limit us in three small places and the renderer under them in one large one.

The large one is not in any proposal. **An author cannot declare a pass.** The order of drawing is fixed
(`Phase` in `src/compile/context.ts`), every texture is the size of the view (`TextureDesc` in
`src/host/manifest.ts`), every draw sees one camera (`frame` at group 0), and the picture is display-referred 8-bit
(`COLOR_FORMAT = "rgba8unorm"`). So shadow maps, planar mirrors, reflection probes, a depth pre-pass for a custom
effect, any float buffer and any accumulation of samples are ruled out, whatever the proposals say. The striking fact
is that **the file format already is a frame graph**: a manifest's frame is a list of render passes over a list of
textures, and "a loader executes it; it decides nothing". The box is only on the compile side, plus two missing
descriptor fields.

The three small ones, each a change to a proposal before it is built:

1. **Materials: a surface's own answer should be a function of the light's direction, not of the style's finished
   colour.** Add one reserved paint key, `scatter: (ctx, toward) => colour` (the share of light arriving from
   `toward` that leaves toward the eye, cosine included), read through `ctx.scatter(toward)`, which falls back to a
   standard response built from `base`, `gloss` and `metal`. A stylised style calls it once per light and prints the
   result in its own medium; an integrator calls it once per sample. The wrap `shade(ctx, styled)` stays, demoted to
   what it is good for: changes to the picture, not to the surface. Fix the units now: `emits` and light colours are
   linear and unbounded; `base` is sRGB as written, linearised by any reader that needs radiance.
2. **Light: a light must say how far and how big it is, and take a sample.** `(p, xi?) => { toward, colour,
   distance, size }`. `distance` ends a shadow ray; `size` is the cone a soft shadow and a glossy highlight need;
   `xi`, when given, picks a point on the light's area so stochastic soft shadows and a path tracer can sample it. A
   light may also be a **family** (one function over an index, each member with a reach), which is what clustered
   culling needs; a list of distinct functions can never be looped over per tile.
3. **Geometry: a hit must say what it hit, and every drawable must say what it is.** `castRay` returns the thing,
   the copy and the body coordinate it met, not only a mean colour, so a reflection or a bounce can evaluate the real
   paint and `scatter` there. And `Drawable.surfaces` (today's two bakeable kinds, told apart by a `"mesh" in s` test)
   becomes `Drawable.forms`, a sum type whose variants are a surface, a mesh, a field, a medium and **a hit function of
   your own**. That one seam lets a second renderer, the baker, the CPU checks and a differentiable reader all read
   what a thing *is* without knowing how this renderer *draws* it.

(State needs one sentence, not a change: a step may never read a rendered pass, so baked state can never become
pixel history in disguise.)

**The one new piece: `pass`**, a picture drawn off screen this frame, from a camera that is an expression, into a
target of any size, read back by any expression with `.at(uv)` and projected into exactly by `.project(p)`, which
also runs on the CPU. Passes are not registered anywhere: like tables, they are found from the expressions that read
them, scheduled before their readers, and absent from the files of every scene that does not use one. With it a
shadow map is about 25 lines of a style part, and a mirror pool is about 15 lines of a scene. With three render
flags and no new name, the same machinery accumulates many samples of one instant into a float target: soft
area shadows, depth of field, antialiasing and **exact motion blur** (samples spread over a shutter, because time is
an argument), with any frame still drawn alone.

**Numbers.** A scene without a pass: byte-identical WGSL, manifest and pixels. A 2048² depth-only shadow map on the
largest example: +0.4 to 1.0 ms in the fast clock state (*estimate*, §12). A full-resolution mirror pass there: +1.0
to 1.8 ms. A 64-sample still of the same scene: about 64 frames, 0.14 s fast. Library: about 650 lines of TypeScript
and 90 of Rust for the pass and still mode, about 300 more for the amendments; two exported names (`pass`, and
`standard` from A1). A path tracer over the same description is possible and
belongs in a separate renderer: *estimate* 5 to 10 ms per sample per frame for a small scene and 30 to 80 ms for the
largest, so a still, never a game frame.

**The context budget does not survive the five proposals as written.** `loft context` is 2,251 words today; the
three proposals add about 281, the light proposal perhaps 60, this one 25. §11 proposes cuts that bring the total back
to about 2,330.

**Swapping the renderer**, in one line per level: replacing how one thing is drawn is a 200-line cliff today and
becomes a `hit` function with the geometry proposal; replacing the look is a `Style` value and is done; replacing the
host is the artefact as files and is done; replacing the renderer wholesale is impossible today, because a drawable
only knows how to emit shaders for this rasteriser, and becomes possible through `forms`. It should be modular at
exactly those four seams and nowhere inside them (§9).

## 2. The review: the four proposals against state-of-the-art practice

What is true today, checked in the code: surfaces are rasterised by vertex pulling, distance fields, relief and
simple solids are traced per pixel inside each thing's bound, first rays only (`src/draw/`); a style is a per-fragment
program plus full-screen passes over the frame's colour and depth (`src/style.ts`, `src/style/post.ts`); the stages
run in a fixed order (`PHASES`); textures are view-sized (`TextureDesc`); the picture is `rgba8unorm`, written as
display sRGB, 4x multisampled (`SAMPLES = 4`); nothing carries over between frames.

**Hardware ray tracing.** No shipped browser WebGPU has a ray tracing or ray query interface as of this date; I
found none in Chrome's WebGPU release notes through mid-2026, and the only browser-side efforts are a compute-shader
emulation (WebRTX) and an unofficial Dawn fork. Native `wgpu`, which the Rust loader uses, has ray queries behind an
experimental, unsafe-to-enable feature (`EXPERIMENTAL_RAY_QUERY`, WGSL `enable wgpu_ray_query`); ray tracing
pipelines are still in development there. Loft's command line draws through Dawn, so **nothing here may depend on
hardware rays**. The `forms` seam (§9) is what would let a native loader use them later without the scene changing.

Columns: **works** as the proposals are designed; **small addition** (named); **ruled out**, and what to change. "P" is
the new `pass` (§5). Amendments A1 to A4 are §6.

| Technique | Works as designed | Needs a small addition | Ruled out by the design; the change |
|---|---|---|---|
| **Many lights** (tiled or clustered culling) | A handful of lights, each a term in every lit shader (light sketch). | **A light family** (A2): one function over an index, each member with a reach (a sphere). Then a per-frame map table builds per-tile lists: 40 x 25 tiles of 32 px at 1280x800 is 1,000 tiles; against 256 lights that is 256k sphere tests, one dispatch (*estimate*: under 0.05 ms). The shader loops over its tile's list. | A list of distinct light functions cannot be culled: each is different code, so a tile cannot loop over "its" lights. Families fix it. |
| **Deferred shading** | Not needed: forward shading with per-thing modules is what the materials proposal builds. | The materials proposal already records **the set of channels the style reads** (its step 1). That finite set is a G-buffer layout for that scene and style, so a deferred path is derivable without a fixed channel list. | A thing's own wrap or `scatter` in a deferred lighting pass needs a thing id and a switch over every thing with its own code, or one stencilled full-screen pass per such thing. Cost grows with the number of wrapped things, and deferred defeats 4x MSAA (4x the G-buffer). **Leave deferred alone** (§10); clustered forward gets the many-light benefit. |
| **Shadow maps**, cascades, filtered (PCF, PCSS) | No: no pass, no off-screen target. The style's shadow block traces each opaque pixel against skeleton capsules, which is exact for round chains and blind to every other shape. | **P**: a depth pass from a camera that follows the sun, read with `.project` and `.at` (§3.1). Cascades are three passes. PCF and contact-hardening (PCSS) are expressions over `.at` (§3.1 is a 3 x 3 PCF). A hardware comparison sampler is a later descriptor field. | Virtual shadow maps (page tables filled from feedback of the previous frame) need history: never (§10). |
| **Ray-traced shadows** | Yes, approximately: `castRay` against capsules plus the ground; exactly against one field (`march`) or one author tracer (`hit`). | Light `distance` and `size` (A2), so the shadow ray ends at the light and its cone is the light's apparent size. | Exact shadows of every rasterised surface: the geometry proposal says no, because a per-frame hierarchy needs a sort. For Loft's own surfaces it does not (§7.2: the parameter grid is the hierarchy). Offline first. |
| **Area lights, soft shadows** | Soft shadows from the closest-approach cone (`gap / width`) of any tracer: a penumbra in closed form, no noise. | A2's `size` (the cone) and `xi` (a sampled point, for still-mode accumulation or an integrator). Glossy response to an area light by widening the lobe by the light's size (representative point, inherited). | Exact area-light shading of an arbitrary response (linearly transformed cosines need a fitted table per response; an open `scatter` has none). In a still, sampling is exact. |
| **Screen-space reflection** | The `reflection` post pass, for water only. | A post pass can march today; it lacks normals (it reconstructs them from depth) and HDR. With P, a style can render a normals pass if it wants exact ones. | Reflections of what is off screen: inherent to the method. |
| **Planar reflection** | No. | **P** with a mirrored camera (§3.2). Oblique near-plane clipping (inherited, Lengyel 2005) is inside that camera variant. | |
| **Ray-traced reflection** | Sky only (`glossed`), or one field or tracer exactly, or every thing as a blob in its mean colour (`castRay`). | A3: a hit says which thing, copy and body coordinate it met, so the reflection evaluates that thing's real paint there. | Reflections of exact rasterised surfaces in real time: as for shadows. |
| **Ambient occlusion** | Closed-form occlusion of spheres and capsules (a relative named by the geometry proposal); screen-space AO is a post pass today (depth is readable). | Nothing. A bent normal from capsules is an expression. | |
| **Global illumination**, probes | No. | Probes rendered **this frame** are P with six cameras into a small target (a 32² cube per probe, 6 passes). Irradiance from capsules (`castRay` cones) is an expression. | Probes that converge over frames (the common real-time method) need history: never. Probes baked once at build for a static part of a scene: possible as a build-time pass, not proposed. |
| **Global illumination**, traced | No. | A separate renderer over `forms` (§7). | In real time, ruled out by cost on this hardware (§7.3), not by the design. |
| **Temporal accumulation, denoising** | No, by design: nothing carries over. | **Still mode** (§3.3): many samples of one instant with fixed seeds, averaged into a float target. Spatial denoising (an edge-aware blur) is a post pass over float targets from P. | Reprojection with history (TAA, temporal denoisers, temporal upscaling): never in the scene or the artefact (§5.4). |
| **Anti-aliasing** | Better than the usual: 4x MSAA plus analytic coverage from `gap / width` for every tracer (an exact edge, no history). | Still mode adds sub-pixel jitter per sample for the stills that want more. | TAA: never; it is not needed. |
| **Physically based surface response** (evaluate and sample a function of in and out directions) | No: `shade(ctx, styled)` answers after the style, with no light direction. | **A1**: `scatter(ctx, toward)` on the paint and `ctx.scatter`. Its default, from `base`, `gloss`, `metal`, is a standard microfacet response that an integrator knows how to sample. | Importance sampling of an author's own `scatter`: an integrator can only sample it by the cosine and weigh it against the lights (multiple importance sampling). Glossy own responses are noisy. A `sample` partner key is a relative, added on first need. |
| **Participating media** | `draw.density`: Gaussian blobs integrated in closed form along the view ray; clouds take point flashes. | A `medium` form (A3) so a second reader sees the density as a function. A light's occlusion through a medium is the same closed-form integral along the shadow ray. | Multiple scattering in real time. |
| **Subsurface scattering** | Stylised: a channel a style reads (wrap lighting, a translucency term from the light's side). | `scatter` can include a smooth wrap term. | True subsurface (a random walk below the surface): offline only. |
| **Thin-film interference** | The beetle in the materials proposal, in three wavelengths. | Written as `scatter` (A1), it answers each light at the right half-vector angle, not only the view angle. | |
| **Diffraction** (gratings, pits, feathers) | The grating equation per wavelength is a closed form in `scatter`: exact, noise-free, per light. Loft is unusually good at this (§8). | | Wave-optical effects across the scene (diffraction around edges, speckle): never. |
| **Dispersion** | Refraction of the sky at three indices of refraction (three closed-form rays) in a wrap or `scatter`. | | A spectral integrator (refraction rays at a sampled wavelength through the scene) needs a wavelength in the shading context: §8 leaves room, adds nothing now. |
| **Caustics** | Closed-form caustic patterns a light can compute (a light that is a function can be the focused pattern under a known swell). | | Caustics found by tracing: hard even for path tracers (needs light tracing or photons); offline only, and not proposed. |

Reading the table by column: **the design rules out only what needs history, and real-time global tracing of
everything**, and the second is a cost, not a door. Every other technique is either already expressible or one named
addition away, and the additions concentrate in two places: the pass (eight rows) and the light's shape (four rows).

## 3. What an author writes with the new piece

Three scenes, written before the design in §5 and then read cold. The first two use `pass`; the third uses still
mode, which needs no new name in a scene. Lights in the third follow the sketch in `docs/open_design.md` with the
shape amendment A2, since the light proposal did not exist when this was written.

### 3.1 Shadows from every shape the sun sees (a style part)

```ts
// Reeds over a floor, toon, with shadows from a depth picture taken from the sun: every shape casts, not only the
// skeletons' capsules, and a reed shades its neighbour. The picture covers a 12-unit square round the origin.
import { add, f, le, mix, mul, normalize, pass, plane, scene, select, sin, strand, sub, time, toon, vec2, vec3, type F, type P3, type Style } from "../src/index.ts";

const SUN = normalize(vec3(-0.4, 0.8, 0.45));
const fromSun = pass({
  name: "fromSun",
  size: [2048, 2048],
  keep: "depth",                                         // only how near the sun each texel's first surface is
  camera: { kind: "look", eye: mul(SUN, 20), target: vec3(0, 0, 0), lens: { halfHeight: 6 } },
});

/** 1 where the sun reaches p, 0 where a nearer surface covers it; nine taps soften the edge over one texel. */
function sunlit(p: P3, normal: P3): F {
  const q = fromSun.project(add(p, mul(normal, 0.02)));  // xy: where p lands in the picture (0..1); z: its depth there
  let lit: F = f(0);
  for (const dx of [-1, 0, 1]) for (const dy of [-1, 0, 1]) {
    const nearest = fromSun.at(add(q.xy, vec2(dx / 2048, dy / 2048))).x;
    lit = add(lit, select(le(q.z, add(nearest, 0.002)), 1, 0));
  }
  return mul(lit, 1 / 9);
}

const base = toon();
const shadowed: Style = {
  ...base,
  name: "shadowed",
  shade: (ctx) => mix(mul(base.shade(ctx), 0.55), base.shade(ctx), sunlit(ctx.world, ctx.normal)),
  ground: (ctx) => base.ground({ ...ctx, shadow: sub(1, sunlit(ctx.world, vec3(0, 1, 0))) }),
};

const reed = (k: number) => strand({
  name: `reed${k}`,
  spine: (m: F) => vec3(add(k * 0.7 - 2, mul(mul(m, m), mul(sin(add(time, k)), 0.5))), mul(m, 3.5), k % 2 ? 0.3 : -0.3),
  girth: (m: F) => add(0.2, mul(m, -0.17)),
  tips: ["open", "point"],
  paint: [0.85, 0.55, 0.2],
});

export default scene({
  ground: plane, things: [0, 1, 2, 3, 4, 5].map(reed), style: shadowed, sun: [-0.4, 0.8, 0.45],
  cameras: { whole: { kind: "perspective", eye: [6, 4, 8], target: [0, 1.5, 0], fovY: 0.6 } },
});
```

`--style woodblock` drops the shadow map with the style that read it: the pass is found from the style's expressions,
so a scene drawn in another style has no such pass in its files.

### 3.2 A pool that mirrors what stands round it (a scene)

```ts
// A still pool in a plane floor: inside 2 units of the origin the ground is a dark mirror, rippled by a closed form
// of time. The mirrored picture is the view's own camera reflected in y = 0, half the view's size.
import { add, dot, length, lt, mix, mul, pass, plane, scene, select, sin, time, toon, vec2, type Style } from "../src/index.ts";
import { crab } from "./scenes/crab_parts.ts"; // any things; the pool does not care what they are

const mirrored = pass({ name: "mirrored", size: { scale: 0.5 }, camera: { kind: "mirror", normal: [0, 1, 0], offset: 0 } });

const base = toon();
const pooled: Style = {
  ...base,
  name: "pooled",
  ground: (ctx) => {
    const r = length(vec2(ctx.world.x, ctx.world.z));
    const ripple = mul(vec2(sin(add(mul(r, 9), mul(time, 2))), 0), 0.004);
    const seen = mirrored.at(add(mirrored.project(ctx.world).xy, ripple)).xyz;
    return select(lt(r, 2), mix([0.05, 0.08, 0.1], seen, 0.85), base.ground(ctx));
  },
};

export default scene({
  ground: plane, things: crab, style: pooled, sun: [-0.4, 0.8, 0.45],
  cameras: { whole: { kind: "perspective", eye: [5, 3, 6], target: [0, 1, 0], fovY: 0.6 } },
});
```

The pass draws every thing with the scene's style from the mirrored camera, clipped at the plane, and leaves out
the ground (whose shader reads the pass): a pass never draws a reader of itself.

### 3.3 A still with an area lamp, soft shadows and motion blur (no new name)

```ts
// A spinning top under a round lamp. In a normal frame the lamp is its centre, and soft shadows come from the cone
// of its apparent size. In a still (`--samples`), each sample takes a point on the disc, and `--shutter` spreads the
// samples over a span of time, so the stripes blur along their motion.
import { add, div, dot, f, fract, length, lt, max, mul, neg, plane, scene, select, sqrt, strand, sub, time, cos, sin, toon, vec3, type F, type Light } from "../src/index.ts";

const C = vec3(0, 3, 0), R = 0.3;
/** A disc lamp facing down, 40 units of radiance. With xi, a point on the disc; the same law gives an exact average. */
const lamp: Light = (p, xi) => {
  const q = xi === undefined ? C : add(C, vec3(mul(mul(sqrt(xi.x), R), cos(mul(xi.y, 6.2832))), 0, mul(mul(sqrt(xi.x), R), sin(mul(xi.y, 6.2832)))));
  const d = sub(q, p), r = length(d), toward = div(d, r);
  const facing = max(dot(neg(toward), vec3(0, -1, 0)), 0);           // the disc's cosine toward p
  return { toward, distance: r, size: div(R, r), colour: mul(vec3(40, 36, 30), div(mul(facing, Math.PI * R * R), mul(r, r))) };
};

const top = strand({
  name: "top",
  spine: (m: F) => vec3(0, add(0.05, mul(m, 0.9)), 0),
  girth: (m: F) => mul(sin(mul(m, Math.PI)), 0.5),
  paint: (m: F, v: F) => select(lt(fract(add(mul(v, 6), mul(time, 9))), 0.5), [0.8, 0.15, 0.1], [0.95, 0.9, 0.8]),
});

export default scene({
  ground: plane, things: [top], style: toon(), sun: [-0.4, 0.8, 0.45], lights: [lamp],
  cameras: { whole: { kind: "perspective", eye: [2.5, 2, 3], target: [0, 0.5, 0], fovY: 0.6 } },
});
```

```sh
loft render examples/top.ts --out out/top.webp                                  # a frame: the lamp's centre, cone-soft shadows
loft render examples/top.ts --out out/top_still.webp --samples 64 --shutter 0.03 # 64 samples: disc points, 30 ms of motion
```

The lamp's estimator is unbiased by construction: with `xi` uniform on the unit square, the point is uniform on the
disc (pdf 1 / (pi R²)), and the colour carries the factor pi R² · cos / r², so the average over samples is the exact
integral over the disc (DERIVATION; the representative point without `xi` is its first-order approximation).

### 3.4 What a stranger asked on reading cold, and what changed

| Question | Defect? | Change |
|---|---|---|
| "Where is `fromSun` registered? I never pass it to `scene`." | No: by design. | It is found from the expressions that read it, as tables are. Said in `pass`'s comment, first line. |
| "Is `project(p).z` the same depth `at(...).x` holds?" | Yes, the first draft had `z` as a distance. | Both are the pass's depth, 0 at the near plane to 1 at the far one, as WebGPU's. One convention. |
| "`keep: "depth"`: does the pass shade at all?" | Partly. | No: a depth pass runs each drawable's depth-only fragment entry, which computes coverage and depth and nothing else (§5.3). Said in the type. |
| "Does the mirror show the pool inside the pool?" | No. | A pass leaves out every draw whose shader reads it, and `loft` reports the names. |
| "Why is there a `sample` input in the third scene?" | Yes: the first draft exported `sample` so a scene could jitter by it. | Removed. The lamp gets `xi`, the camera's jitter and aperture belong to the still mode, and time is already an argument. One export fewer. |
| "Can `at` run in a check?" | No defect. | `.at` is GPU-only (reported as `loft_fwidth` is today); `.project` is an expression and runs on the CPU, so "is the whole grove inside the sun's picture" is a check. |

## 4. Options for opening the frame

The question is how an author says "draw these things from this camera into this target, then let an expression read
it", without a menu and without breaking the three things the frame owes everyone: byte-identical pictures from a
fresh process, the artefact as files that a loader executes without deciding anything, and restyling.

| Family | What a user writes | What it cannot express | What it costs |
|---|---|---|---|
| **Special types** | `shadowMap({ light, size, cascades })`, `planarReflection({ plane })`, `cubeProbe({ at })` | Everything off the menu: a pass from a camera that follows a creature's eye, a normals picture for a custom ink, a picture read by a light, a probe at a moving point. | A module per technique (shadows alone are four: plain, cascaded, PCF, PCSS), each with a style-side reader. Every style must know every kind. Breaks rules 1 and 4. |
| **Parameters only** | `scene({ shadows: { size: 2048 }, mirror: { plane } })` | The same, plus any reading other than the library's. | Smaller, but every new reading is a release. |
| **A render-graph object** (the engine shape) | `graph.addPass({ inputs, outputs, execute(enc) })` with resources declared, ordered by the author | Nothing, on the GPU. | Hands the author the encoder: no CPU twin, no restyling (the graph is not a function of the style), the order of calls becomes state, and the loader must run author code, which the files cannot carry. Breaks "a loader decides nothing". |
| **Shaders only** | Raw WGSL `PostSpec` plus host changes | Off-screen targets and second cameras: a post pass cannot draw geometry. | Exists for full-screen passes only; the host would need a new hook per technique. |
| **Passes as expressions** (recommended) | `pass({ name, size, keep?, camera })` returns a value whose `.at(uv)` and `.project(p)` are expressions; nothing is registered. | A pass whose contents depend on the previous frame (by design); a pass that draws something other than the scene's things or a full-screen expression (a custom draw list is the `only` field, a later addition on first need); compute passes that scatter (the state proposal's rule). | One export. A camera computed in the pose pass; fixed-size textures and one more bind group in the format; a depth-only fragment entry per drawable kind. About 650 lines TS, 90 Rust (§12). |
| **Something subtler: passes as tables** | Treat a rendered picture as one more table kind, `fetch`ed like any other | The same as the row above. | The table machinery (levels, slots, `varianceOf`) assumes buffers filled by compute and evaluable on the CPU; a picture is neither. Forcing it in would add a GPU-only table kind that every table reader must special-case. Rejected, but the recommendation borrows its one good idea: **discovery from expressions**. |

Why the recommended row wins, and what is mine in it:

- **Discovery, not declaration** (mine, borrowed from how Loft already finds per-frame tables). A pass exists in the
  files exactly when some emitted expression reads it. A style that reads a shadow map brings it; `--style` to one
  that does not removes it; a scene with no reader is byte-identical. The order is not written by anyone: a pass is
  drawn before every draw that reads it, and passes that read passes are ordered by the same dependency levels as
  tables. A cycle is refused by name.
- **A camera is an expression** (mine). The pass's view-projection is computed in the pose pass from expressions
  (the sun, a creature's eye, the view's own camera reflected) into a buffer that is then bound as that pass's
  `frame`. The loader computes nothing new: it runs one more dispatch it already knows how to run. Every pass reuses
  every drawable's pipelines, because a drawable already reads its camera from `frame`, including `frame.viewport`
  for pixel footprints and coverage, so ink widths and trace coverage come out right in the pass's own pixels.
- **`project` runs on the CPU** (mine). A pass's camera is an expression, so where a world point lands in it is
  arithmetic: a check can measure that every caster is inside the sun's picture, or how many texels a reed's width
  spans, before anything is drawn. Shadow-map acne and coverage are measurable, not looked for.
- **A reader is never drawn into what it reads.** This one rule makes mirrors and shadow maps safe without asking
  the author to list anything (below).

Inherited: render-to-texture, shadow mapping (Williams 1978), percentage-closer filtering (Reeves et al. 1987), planar
reflection by a mirrored camera with an oblique near plane (Lengyel 2005), frame graphs that order passes by declared
reads (as in production engines).

## 5. The recommendation: `pass`, and still mode

### 5.1 Types

```ts
// src/frame/pass.ts (new, about 220 lines with its comment). Exported from src/core.ts.

/**
 * How a pass looks at the world. A mechanism, so a closed list, with "your own" as the last variant (rule 4).
 * - look: from `eye` toward `target`; `fovY` (perspective, radians) or `halfHeight` (orthographic, world units).
 * - mirror: the view's own camera reflected in the plane dot(normal, p) = offset, clipped at it (an oblique near
 *   plane), so nothing behind the mirror shows in it.
 * - matrix: your own view-projection, four columns, and the eye it is from (for view directions and pixel sizes).
 * Every field is an expression: a camera may follow the sun, a creature's eye or the view; it is computed once a
 * frame on the GPU, and on the CPU for `project`.
 */
export type PassCamera =
  | { readonly kind: "look"; readonly eye: In<"vec3">; readonly target: In<"vec3">; readonly up?: In<"vec3">;
      readonly lens: { readonly fovY: In<"f32"> } | { readonly halfHeight: In<"f32"> }; readonly near?: number; readonly far?: number }
  | { readonly kind: "mirror"; readonly normal: Rgb; readonly offset: number }
  | { readonly kind: "matrix"; readonly columns: readonly [In<"vec4">, In<"vec4">, In<"vec4">, In<"vec4">]; readonly eye: In<"vec3"> };

/**
 * A picture drawn off screen this frame. Nothing registers it: it is in the scene's files exactly when an expression
 * that is drawn reads it (`at`), and it is drawn before every draw that reads it. A draw that reads a pass is never
 * drawn into that pass (`loft` notes which were left out).
 *
 * - size: the view's size, a share of it, or fixed pixels.
 * - what: `scene` draws the scene's things from `camera`; `screen` is a full-screen expression of the pixel's uv
 *   (0..1), which may read other passes (a blur, a smaller copy, one level of a bloom chain).
 * - keep (scene only): "colour" (default) is the picture as the style draws it, through the view's own pipelines;
 *   "depth" runs each thing's depth-only fragment (coverage and depth, no shading) and keeps the depth, 0 near to 1
 *   far; "light" keeps colour in 16-bit float, for a style that writes values above 1.
 *
 * Cannot: read the previous frame (nothing carries over); scatter; draw a list of things other than the scene's
 * (`only` is a later addition on first need); be read on the CPU (`at` is GPU-only; `project` is not).
 * Going past it: a `PostSpec` reads passes by name (`reads`), in WGSL. Relatives: cascades (three `look` passes over
 * nested squares); a normals picture for a custom ink (a `scene` pass under a style whose shade returns the normal);
 * six `look` passes as a probe's cube.
 */
export type PassSpec = {
  readonly name: string;
  readonly size: "view" | { readonly scale: number } | readonly [number, number];
} & (
  | { readonly what: "scene"; readonly camera: PassCamera; readonly keep?: "colour" | "depth" | "light" }
  | { readonly what: "screen"; readonly colour: (uv: Expr<"vec2">) => Expr<"vec4"> }
);

export type Pass = {
  readonly name: string;
  /** The picture at uv (0..1): colour, or the depth in x. Bilinear for colour, nearest for depth. GPU-only. */
  at(uv: In<"vec2">): Expr<"vec4">;
  /** Where a world point lands: xy its uv (0..1), z its depth (0 near to 1 far). An expression: runs on the CPU. */
  project(p: In<"vec3">): Expr<"vec3">;
};

export function pass(spec: PassSpec): Pass;
```

The scenes in §3 omit `what: "scene"` for brevity; in the build it is the default when `camera` is given, so they
run as written. `Rgb` is the existing `readonly [number, number, number]`.

### 5.2 Where it plugs in

| Piece | Where | Lines (estimate) |
|---|---|---|
| `pass`, `Pass`, the camera's view-projection as expressions (look-at, perspective or orthographic, the mirror's reflection with an oblique near plane); `.at` as a GPU-only `LibFn` naming the pass; `.project` as plain expressions | `src/frame/pass.ts` (new) | 220 |
| Discovery: collect the passes named by the `LibFn`s of every emitted shader and post spec; order passes by what they read (dependency levels, as tables are); refuse a cycle by name; the left-out list per pass | `src/compile/passes.ts` (new) | 120 |
| The camera: lifted by the existing rates machinery into a frame-values computation, written by one dispatch into a 48-float buffer laid out as `struct Frame` (with the pass's own `viewport`) and bound as that pass's group 0 | `src/compile/tables.ts`, `src/host/lower.ts` | 60 |
| Lowering: per pass, its textures, its group-0 bind group (its own `frame`), a group-3 bind group with every other pass's texture (a 1 x 1 stand-in in its own place, because WebGPU forbids a texture as attachment and binding in one pass), and its draw commands: the view's commands minus readers; for `depth`, the depth pipelines | `src/host/lower.ts` | 150 |
| Group 3 declarations prepended to modules and post passes that read passes (as `SCENE_DEPTH_WGSL` is for group 2) | `src/artefact.ts` | 30 |
| Format: `TextureDesc.size` becomes a sum (below); `Draw.depth` (5.3); manifest version 2 only when a pass exists | `src/host/manifest.ts`, `docs/artefact_format.md` | 40 |
| Rust loader: fixed and scaled textures, the extra bind groups and passes (it already runs dispatches and passes from lists) | `rust/loft-loader` | 90 |
| Still mode in the TS host and the command line (5.4) | `src/host/host.ts`, `src/cli/` | 70 |

```ts
// src/host/manifest.ts: TextureDesc gains a size; today's is { kind: "view", scale: 1 }.
export type TextureSize = { readonly kind: "view"; readonly scale: number } | { readonly kind: "fixed"; readonly width: number; readonly height: number };
export type TextureDesc = { readonly label: string; readonly size: TextureSize; readonly format: GPUTextureFormat; readonly sampleCount: number; readonly usage: readonly TextureUsage[] };
```

Why a colour pass reuses the view's pipelines: a render pipeline is fixed by its shader, its target formats and its
sample count. A colour pass renders into `rgba8unorm` with 4 samples and `depth32float`, exactly as the view does, and
resolves into a single-sample texture for reading. So it creates **no new pipeline**, and adds nothing to the load
time, which on the largest example is dominated by pipeline creation. A colour pass at half the view's size costs 640
x 400 x 4 samples x 8 bytes = 8.2 MB plus its 1 MB resolve.

### 5.3 The depth-only fragment, and the one rule about readers

A shadow map that ran every thing's full shading would pay for colour nobody sees: on a traced field that is the
march **and** the style. So a draw states how it draws depth, as a sum type, and the compiler dispatches on it:

```ts
// src/artefact.ts, Draw gains:
/** How this draw writes depth alone, for a "depth" pass. Absent: it is left out of depth passes, and `loft` notes it. */
readonly depth?:
  /** No fragment stage: the rasterised triangle is the depth (surfaces, meshes). */
  | { readonly kind: "vertex" }
  /** A fragment entry that computes coverage and depth and discards, without shading (every tracer). */
  | { readonly kind: "entry"; readonly entry: string };
```

The built-in surfaces state `vertex`. The shared tracer fragment (`src/draw/trace.ts`, and `draw.traced` in the
geometry proposal) gains `fs_depth`: the twelve shared lines minus paint and shade. A drawable written by an author
inherits it through `draw.traced`; a hand-written `Drawable` that states nothing is simply left out, never wrong.

**The rule about readers.** A draw whose module reads pass X is not drawn into X. In a depth pass this almost never
bites, because a depth entry does not shade and so reads no pass; a depth entry that does read X (a shape whose
geometry depends on its own shadow) is a true cycle and is refused. In a colour pass it is exactly what a mirror needs:
the mirror surface is left out of its own reflection.

### 5.4 History, and still mode

Three options (question 3 of the brief):

| Option | What it buys | What it costs |
|---|---|---|
| **Never** | Every property Loft has. | No accumulation at all: soft area shadows only as cones, no exact motion blur, AA only from MSAA and coverage. |
| **Still mode** (recommended): many samples of one instant, fixed seeds, averaged | Area lights sampled exactly, depth of field, extra AA, and motion blur across a shutter; each frame still a function of its time and its sample count, byte-identical from a fresh process. | N times the frame, for stills and clips only. A separate compile, because lights are called with `xi` (below). |
| **Opt-in history** (TAA, temporal denoising, probes that converge) | Real-time quality at a fraction of the cost per frame. | Gives up any frame drawn alone, parallel and out-of-order clips, checks at an instant, and byte-identical pictures (they depend on the frames before, and on frame timing). |

**Recommendation: still mode now; opt-in history never in a scene, an artefact, a check, a clip or a variant.** If a
game that loads Loft's files wants temporal antialiasing on its own display, that is the game's renderer on top of
the files and nothing of it flows back. What I would never do: let any picture `loft` makes depend on an earlier
frame; ship reprojection heuristics in the core; let baked state read a rendered picture (that is history smuggled
through `recur`, amendment A4).

Still mode, exactly:

- `loft render`, `frames` and `clip` take `--samples N` (default 1: today's frame), `--shutter s` (default 0) and
  `--aperture r` (default 0; the focus is the camera's target).
- Sample k (0..N-1) is drawn at time t + s (k + 0.5) / N, with the projection offset by the k-th point of the (2, 3)
  Halton sequence inside the pixel, and the eye moved to the k-th point of a disc of radius r about it, aimed at the
  same target. Lights receive `xi` = the k-th point of the R2 sequence, rotated per pixel by a hash of the pixel
  (Cranley-Patterson rotation, inherited). All of it is a function of (t, k, pixel): no state.
- Each sample's picture is resolved as today and added into an `rgba32float` accumulator by a full-screen pass; the
  average then goes through the style's own post passes once (bloom, grain, ink from depth use the depth of sample 0).
- Still mode compiles the scene with `xi` given to lights, so a normal frame never contains the sampled branch and
  stays byte-identical. Still mode lives in the TypeScript host and the command line only: the file format and the
  Rust loader do not change for it, since a game draws frames, not stills.
- The time spread gives **exact motion blur with no motion vectors** (mine): because time is an argument, sample k
  is simply the scene at its own time. Engines estimate motion blur from velocity buffers because they cannot ask
  for the scene at another time; Loft can.

### 5.5 What stays byte-identical

- Every scene that reads no pass: WGSL, manifest (version 1) and pixels, through every stage of the build.
- Every normal frame of a scene in still mode's absence (`--samples 1`).
- Restyling: passes come and go with the expressions that read them; a scene's own passes (the pool) draw with
  whatever style is chosen.
- The CPU checks: `.project` evaluates; `.at` is reported as GPU-only, as `loft_fwidth` is today.

## 6. Amendments to the four proposals

Each is a precise change to a type, with the reason. None changes what an existing scene writes; each keeps its
proposal's byte-identity claims, by the same "default is the old node" device the materials proposal uses.

### A1. Materials: `scatter`, a response to a light's direction, and units

```ts
// src/map/types.ts, Painted gains one reserved key (beside `base` and `shade`):
  /**
   * How this surface answers light from `toward` (unit, from the surface toward the light): the share that leaves
   * toward the eye (`ctx.view`), cosine included, per colour channel. A style calls it per light and prints the result
   * in its own medium; an integrator calls it per sample. Prefer it to `shade` for anything that is a property of the
   * surface (a film, a grating, a sheen, brushed metal); keep `shade` for changes to the picture.
   */
  readonly scatter?: (ctx: ShadeCtx, toward: Expr<"vec3">) => P3;

// src/style.ts, ShadeCtx gains one member (beside `channel` and `sky`):
  /** This thing's `scatter` toward `toward` if its paint gives one, else `otherwise` (the style's own term). */
  scatter(toward: Expr<"vec3">, otherwise: In<"vec3">): Expr<"vec3">;

// src/style/standard.ts (new, about 40 lines): the response an integrator falls back to.
/** Lambert plus a Trowbridge-Reitz (GGX) lobe with Smith masking and Schlick's Fresnel, from base (linearised), gloss
 *  (roughness alpha = max(0.02, (1 - gloss)^2)) and metal. Inherited (Walter et al. 2007; Karis 2013). */
export function standard(ctx: ShadeCtx, toward: Expr<"vec3">): Expr<"vec3">;
```

Reasons:

- **The wrap cannot be called by anything but the style that produced `styled`.** A path tracer has no styled
  colour; a style with three lights cannot apply a wrap per light; a reflection ray that meets the thing cannot ask
  it how it looks from there. A function of the light's direction can be called by all of them. This is the answer to
  the brief's question 4: yes, a thing's response is better defined as a function of the incoming direction, and the
  finished-colour wrap is the right tool only for a picture-level change.
- **`otherwise` makes it free** where a thing gives no `scatter`: the call returns the style's own node, so the style's
  WGSL is unchanged, exactly as `channel(name, otherwise)` is.
- **The beetle, rewritten**, answers each light at its half-vector, which a film really does (the view-only version is
  right only for light from the eye's direction):
  `scatter: (ctx, l) => mul(film(dot(ctx.normal, normalize(add(l, ctx.view))), nm), max(dot(ctx.normal, l), 0))`.
  Toon bands the luminance of the sum over lights and keeps its hue; an integrator uses it as it stands.
- **Units, fixed now because they cannot be fixed later without changing every scene that relies on them.** `base`
  stays sRGB as written (styles print it, and every scene writes it so); a reader that needs reflectance linearises it
  at its boundary. `emits` is **linear radiance and may exceed 1**; a style that prints into 8 bits compresses it, as
  toon would. `gloss` maps to a roughness by the formula above, which the comment of `glossed` should state so that the
  sky's reflection and a light's highlight are the same lobe.

Cost: one reserved word in `loft context` (counted in §11), one export (`standard`), one `ShadeCtx` member.

### A2. Light: distance, size, a sample, and families

Working from the sketch in `docs/open_design.md` (`(p) => ({ toward, colour })`), since the light proposal was not
written:

```ts
/** What arrives at p from one light. `colour` is linear and unbounded: the light at p, before the surface's response. */
export type Lit = {
  readonly toward: Expr<"vec3">;  // unit, from p toward the light (the point sampled on it, given xi)
  readonly colour: Expr<"vec3">;
  readonly distance: Expr<"f32">; // to that point: where a shadow ray stops (a large number for the sun)
  readonly size: Expr<"f32">;     // the light's apparent radius from p, radians: a soft shadow's cone, a highlight's widening
};
/** A light. Without `xi`, its representative point; with `xi` uniform in the unit square, a point on its area, with
 *  `colour` already divided by that point's probability, so the average over samples is exact. */
export type Light = (p: Expr<"vec3">, xi?: Expr<"vec2">) => Lit;
/** Many lights that are one function: member i, and a sphere outside which member i is negligible. Culled per tile. */
export type LightFamily = { readonly count: number; readonly light: (p: Expr<"vec3">, i: F, xi?: Expr<"vec2">) => Lit; readonly reach: (i: F) => { readonly centre: P3; readonly radius: F } };

// scene({ lights?: readonly (Light | LightFamily)[] }); a function is one light, an object a family (classified once).
// ShadeCtx gains, instead of an array of lights:
  /** The sum over every light reaching here of `term(lit)`: the style says what one light does, Loft does the loop. */
  lights(term: (lit: Lit) => Expr<"vec3">): Expr<"vec3">;
```

Reasons: `distance` and `size` are what a shadow ray (`castRay`, `march`) needs to be bounded and soft, and what a
glossy highlight from a non-point light needs; `xi` is what sampling needs (still mode, an integrator); families are
the only form clustered culling can loop over; and `lights(term)` is the only form that works for both a fixed list
and a tile's list, while letting a banded style apply its band after the sum. Every addition is optional for the
author: a three-line point light gives `size: 0` and ignores `xi`.

### A3. Geometry: hits that say what they hit, and drawables that say what they are

```ts
// castRay returns, beside t, gap, normal and colour:
  readonly id: Expr<"f32">;  // the thing's ShadeCtx.id (a hash of its name, varied per copy), so id-keyed patterns survive in a reflection
  readonly m: Expr<"f32">;   // where along the hit chain, 0 root to 1 tip (a strand's own m)

// src/compile/context.ts: Drawable.surfaces (Bakeable = SurfaceBakeable | MeshBakeable, told apart by `"mesh" in s`)
// becomes a sum type every reader dispatches on exhaustively:
export type Form =
  | { readonly kind: "surface"; readonly name: string; readonly surface: Surface; readonly paint: Paint; readonly grid: Grid; readonly sides: "one" | "two"; readonly copy: number }
  | { readonly kind: "mesh"; readonly name: string; readonly mesh: TriangleMesh; readonly paint: (hit: { readonly p: P3; readonly normal: P3 }) => P3 | Painted }
  | { readonly kind: "field"; readonly name: string; readonly field: Field; readonly bound: TraceBound; readonly paint: (hit: FieldHit) => P3 | Painted }
  | { readonly kind: "medium"; readonly name: string; readonly bound: TraceBound; readonly density: (p: P3) => F; readonly colour: (p: P3) => P3 }
  /** Your own: any solid you can intersect. Every other kind can be lowered to this one by a reader that wants to. */
  | { readonly kind: "hit"; readonly name: string; readonly count: number; readonly bound: TracedOptions["bound"]; readonly hit: (ray: Ray, copy: F) => Traced; readonly paint: TracedOptions["paint"] };

export interface Drawable {
  readonly name: string;
  draws(ctx: CompileContext): Emitted;          // how THIS renderer draws it
  readonly skeletons: readonly Skeleton[];
  /** What it is, for any other reader (the baker, the checks, another renderer). Empty: nothing a reader can use. */
  readonly forms: readonly Form[];
}
```

Reasons: today a second reader cannot see a traced field's distance function, a medium's density or an author's
tracer at all (verified: `FieldDrawable.surfaces` returns `[]` for the trace and brick schedules, and
`density`, `copies`, `impostors` and `glow` return `[]`; the field function is a public property of the
`FieldDrawable` class but not of `Drawable`, so reaching it would take an `instanceof` test). The kind test
`"mesh" in s` in `src/bake/mesh.ts` is the dispatch this replaces. The geometry proposal's `draw.mesh` and
`draw.traced` give a `mesh` and a `hit` form; `draw.field` gives `field`; `draw.density` gives `medium`. Cost: about
150 lines, mostly moving what each drawable already holds into one member; the baker's dispatch becomes a switch.
No word in `loft context` (the `Drawable` level is for drawable authors).

### A4. State: a step never reads a pass

One line in the state proposal's "what a step may read": **a pass is refused**, naming it. A recurrence that read a
rendered picture would make the baked trajectory depend on rasterisation on the baking GPU, and would be the door
through which frame history comes back (a probe "converging" over steps). Nothing else in the state proposal
constrains headroom: it is orthogonal to light.

## 7. A physically based mode

**Can a path tracer read the same scene description?** After A1 to A3, yes, and nothing it needs would be a second
copy of the scene: shapes from `forms`, surface response from `scatter` (or `standard` over `base`, `gloss`, `metal`),
emitters from `lights` (sampled with `xi`) and from paint's `emits` (met by rays), the environment from the style's
`sky`, which is a closed-form function of direction and so an exact environment light with no map and no resolution
(the materials proposal's observation, which pays twice here). Before A1 to A3 it cannot: a traced field, a medium
and an author's tracer are invisible to any reader but this rasteriser, the only response is a finished colour, and a
light has no area to sample.

### 7.1 Three routes

| Route | How | For Loft's things | Verdict |
|---|---|---|---|
| **Everything as distance, marched** | One scene distance, sphere-traced per bounce | Fields: exact. Swept and parametric surfaces (strands, sheets, lathes, most of what Loft draws) have **no distance function**; the nearest one is the skeleton's capsules, which is the wrong shape, or a closest-point solve on (u, v) per step, 10 to 20 surface evaluations a step. | Rejected as the main route; right for fields. |
| **Triangles in a hierarchy built in shader code, other forms as leaves** | Posed triangles of every surface and mesh in a two-level hierarchy; a leaf may be a field's bound (marched with `march`), an author's `hit`, or a medium's bound | All forms, each read exactly as it is | **Recommended.** |
| **Hybrid with a rasterised first hit** | The first hit from a G-buffer pass (P), bounces traced | Saves one ray of about seven per sample | A later optimisation, not a route. |

### 7.2 The hierarchy needs no sort, for Loft's own surfaces (mine)

The geometry proposal rules out exact rays against rasterised surfaces because a per-frame hierarchy over moving
triangles needs a sort, which Loft's table programs (map, scan) cannot express. That is true of triangle soup. It is
not true of Loft's surfaces: **every surface is a map from a fixed (u, v) grid, so the grid's quadtree is already a
hierarchy**, fixed in topology for the life of the scene. Per frame only its boxes change:

- A leaf box is the box of a cell's four posed corners, and that box is exact for the cell's two triangles (a
  triangle lies in the hull of its corners). One map table over cells.
- A parent's box is the union of its four children's: one map table per level, log4 of the cell count levels.
- On the largest example (125k triangles, about 62k cells, 8 or 9 levels) that is about 10 dispatches at the
  measured 5 to 15 us each: **0.1 to 0.2 ms per frame, with no sort** (*estimate*).
- Its quality is the open risk: siblings overlap where a map folds back on itself (a coil, a curled fin), and overlap
  costs traversal steps. A continuous map keeps neighbours in (u, v) near in space, which is what a hierarchy needs;
  how near depends on the map's stretch, which `loft profile` could report per surface. CONJECTURE until measured
  against a sorted hierarchy built on the CPU for the same frame.
- The top level is one box per drawn instance (about 100 on the largest example), tested by brute force or by the
  chain spheres the geometry proposal already builds for `castRay`.

`draw.mesh` triangles from the author's own code have no grid; they take a hierarchy built once at build on the CPU
(sorted, of the rest mesh) and refitted per frame through `at` by the same level-by-level unions.

### 7.3 Cost per frame (estimate, and how to measure it)

Per sample per pixel: one primary ray, three bounces, one shadow ray per bounce toward a sampled light: about 7 rays.
At 1280 x 800 that is 7.2M rays per sample.

| Scene | Rays per second (software traversal, this machine; CONJECTURE) | Per sample | 256 samples (no denoiser) | 16 samples + a spatial denoiser |
|---|---|---|---|---|
| Small (the reed: a few thousand triangles, a plane, a sky) | 0.7 to 1.4 G | 5 to 10 ms | 1.3 to 2.6 s | 0.1 to 0.2 s |
| Largest example (125k triangles, 48 chains, fields) | 90 to 240 M | 30 to 80 ms | 8 to 20 s | 0.5 to 1.3 s |

So a path-traced Loft frame is a still or an offline clip (a 10 s clip at 30 frames a second and 256 samples: 40 to
100 minutes on the largest example), never a game frame on this hardware. A second cost to watch: hits must evaluate
the paint and `scatter` of whatever they meet, so one module holds every thing's paint (the largest example's
per-thing module code is 1.45 MB of WGSL today); its compile time may dominate a short render.

**How to measure before building anything:** a throwaway compute shader under `out/` that traces the largest
example's triangles at one time (taken from the bake, which already writes them) through (a) the parameter-grid
hierarchy and (b) a sorted hierarchy built on the CPU, with flat-coloured hits, counting rays per second with GPU
timestamps over 30 frames. The table above is scored against it. Two days.

### 7.4 A style, a second renderer, or a separate tool?

**A second renderer, kept as a separate tool, built only on a second user's need.** Not a style: a style is a program
inside a rasterised frame, and a path tracer owns the whole frame (compute dispatches, an accumulator, its own
emitter sampling). Not inside `compile`: `compile` is one function from a scene to an artefact for this renderer; the
path tracer is another function from the same scene, and the `forms` seam (A3) is what makes that possible. What the
core does **now** is only A1 to A3, so the door stays open at the cost of about 300 lines and one context word. What
a physically based mode honestly loses: `--style` means nothing to it beyond the sky and the post passes, because a
style is a look and a path tracer has none; ink outlines and printed blocks do not exist there.

## 8. Colour

**What three numbers rule out.** Effects whose colour comes from wavelength-dependent geometry across the scene:
dispersion through refraction (a prism's spread becomes three offset copies), a diffraction grating seen in a
reflected ray (three discrete images, banded), thick films (interference that aliases at three samples),
fluorescence, and lights whose spectra make metamers (a sodium lamp turning two equal paints apart). Inside one
surface's own response none of these are ruled out: **a function can loop over wavelengths**. The beetle samples three;
a grating's colours at 16 wavelengths, folded into RGB with an analytic fit of the colour-matching functions (Wyman et
al. 2013), is about 30 operations per wavelength, 500 per fragment, which is 0.5 GFLOP over a million fragments, about
0.1 to 0.2 ms on this GPU (*estimate*). Loft's closed-form, per-light shading is, for once, better placed than a path
tracer here: exact and noise-free.

**What a wavelength in the shading context would cost.** In a raster style: N times the shading arithmetic, for every
fragment. In a path tracer: one hero wavelength plus three rotated companions per path (Wilkie et al. 2014), where
traversal dominates, so about 10 to 30% more per sample.

**Leave room in the types now?** No new type. Two things only: A1 fixes units (`emits` and lights linear and
unbounded, `base` sRGB as written), and the comment of `scatter` reserves an optional third argument, `nm`, for a
spectral reader. Adding an optional argument later breaks no function written without it, so the room costs nothing
today. The other colour limit is not three numbers but **eight bits, display-referred**: light above 1 cannot be held
between passes. `pass({ keep: "light" })` (16-bit float) and still mode's 32-bit accumulator remove it where it
matters; the view's own target stays `rgba8unorm`, so every existing picture is unchanged.

## 9. Swapping the renderer

The owner's question: if someone wants to replace the ray tracing or the marching, swap the renderer, or add
features, does Loft let them, and should it? From the code, level by level:

| Level | What an author writes | Today | With the proposals |
|---|---|---|---|
| **One thing's drawing** (their own marcher for their own solid) | A `Drawable`: `draws(ctx)` returning shaders and `drawSpec`s, raw WGSL or `emitShaderFn`; `wgslFn` for raw code inside expressions | A cliff. `Drawable`, `CompileContext`, `drawSpec`, `emitShaderFn` are exported (`src/core.ts`), but the tracer plumbing is not: `traceWgsl`, `tracePlumbingWgsl`, `traceShadeWgsl`, `outlineWgsl` and `aabbOf` are exported from `src/draw/trace.ts` and not from the core. The geometry proposal measured about 200 lines and the prelude's undocumented eight-argument `shade`. | `draw.traced(name, { bound, hit, paint, outline })`: the author writes only `hit`. A raw marcher goes inside `hit` through `wgslFn` (losing only the CPU twin). It also gets the depth entry (§5.3) and a `hit` form (A3) for free. |
| **The look** | A `Style` value: spread one and replace `shade`, or write all of it | Done. | A style may also own passes (§3.1), and read `scatter` and `lights(term)`. |
| **The host** | A loader of the files: `manifest.json`, WGSL chunks, f32 blobs | Done: "a loader executes it; it decides nothing". The Rust loader is about 1,950 lines plus a 280-line viewer; it has not been re-verified since the ground kinds landed. | Passes add fixed-size textures and one bind group: about 90 lines of Rust. |
| **The renderer, wholesale** (a path tracer, a differentiable renderer, another rasteriser) | Today: a function over `scene.things` that reads `surfaces` and `skeletons` | **Impossible except for surfaces.** Verified: the baker (`src/bake/mesh.ts`) reads `Drawable.surfaces`, and every traced field (trace and brick schedules), `copies`, `impostors`, `density` and `glow` return `[]`. A field's function is reachable only by an `instanceof FieldDrawable` test. What a style means is not readable at all except as code for this rasteriser. | `forms` (A3): a sum type of what a thing is, with "your own `hit`" as a variant. A second renderer is `(scene) => picture` that switches over `forms`, evaluates paint through the materials proposal's `classify`, and reads `lights` and `style.sky`. |

**Where the one seam goes.** Between what a thing **is** (`forms`: a surface, a mesh, a field, a medium, or your own
intersection, each with its paint) and how this renderer **draws** it (`draws(ctx)`). It is one member on
`Drawable`, filled from values each drawable already holds. It serves four readers at once, which is why it is worth
its 150 lines:

- **The physically based mode** (§7) reads every variant.
- **Baking** reads `surface` and `mesh` today and could mesh a `field` by surface nets (which exists) instead of
  skipping it.
- **The CPU checks** could test the exact `hit` or `field` of a named thing, where `staysInView` now uses capsules.
- **Differentiation** (`docs/differentiable.md`): forms are expressions, so `grad.ts` reaches them; the implicit-function
  derivative of a field's hit needs the field, which today no reader but the rasteriser can see.

**What it must not become.** A closed menu of shape kinds that every new idea must join: the `hit` variant is the
author's own function, and any other variant can be lowered to it by a reader that wants to. Nor an abstract
`Renderer` interface with a registry, plug-ins or a scene graph that renderers subscribe to: `compile` stays one
function, and a second renderer is another function. And a reader that meets a form it does not handle reports it
by name (as the baker's `skipped` list does), never falls back silently.

**Should it be modular everywhere?** No. Modular at the four seams above, and nowhere inside them, because inside them
each piece has one right answer and one implementation (rule 3): the order of the built-in phases (passes open the
frame at its edge; the inside stays fixed), a style's mark and printed-block protocol, the level-of-detail planner,
table scheduling and binding, the prelude, the sea's own paths. An interface with one implementation buys no
capability and costs names, words in `loft context`, and indirection a model must read through. Every seam named here
has at least two readers in sight; that is the test.

## 10. What to leave alone

| Practice | Why Loft should never chase it |
|---|---|
| **Temporal reprojection** (TAA, temporal upscaling, temporal denoisers, probes converging over frames) | It trades "any frame draws alone" and byte-identical pictures for frame time. Loft's antialiasing from closed-form coverage already beats it on edges, and still mode gives exact accumulation where quality matters. |
| **Real-time path tracing and real-time global illumination** | 30 to 80 ms per sample on the largest example (§7.3): two orders of magnitude over the frame budget, and the looks Loft exists for (toon, woodblock, print, neon) are decided by a style, not by converged light. |
| **Deferred shading** | Its benefit is many lights; clustered forward gets that without a G-buffer, keeps 4x MSAA cheap, and keeps per-thing `scatter` and wraps in per-thing modules. |
| **Virtual shadow maps, virtual textures, mesh streaming** | Feedback from the previous frame (history), page tables and streaming exist for asset-scale worlds. Loft's scenes are code-scale. |
| **A material graph, a node editor, a material library** | Channels, `scatter` and functions are the vocabulary (rule 1). A menu of materials is the box `docs/open_design.md` exists to avoid. |
| **Physically based camera exposure, tone-mapping pipelines, colour management beyond sRGB** | A style decides how light becomes ink; a single `emits` compression per style is enough. A colour-managed pipeline is a large surface in `loft context` for no look Loft draws. |
| **Hardware ray tracing** | Not in browser WebGPU; experimental in native `wgpu` only. Revisit when it ships in Dawn, through `forms`, without any scene changing. |
