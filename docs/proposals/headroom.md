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
shadow map is about 25 lines of a style part, and a mirror pool is about 15 lines of a scene. With one more input,
`sample`, and two render flags, the same machinery accumulates many samples of one instant into a float target: soft
area shadows, depth of field, antialiasing and **exact motion blur** (samples spread over a shutter, because time is
an argument), with any frame still drawn alone.

**Numbers.** A scene without a pass: byte-identical WGSL, manifest and pixels. A 2048² depth-only shadow map on the
largest example: +0.4 to 1.0 ms in the fast clock state (*estimate*, §8). A full-resolution mirror pass there: +1.0
to 1.8 ms. A 64-sample still of the same scene: about 64 frames, 0.14 s fast. Library: about 650 lines of TypeScript
and 90 of Rust; two exported names (`pass`, `sample`). A path tracer over the same description is possible and
belongs in a separate renderer: *estimate* 5 to 10 ms per sample per frame for a small scene and 30 to 80 ms for the
largest, so a still, never a game frame.

**The context budget does not survive the five proposals as written.** `loft context` is 2,251 words today; the
three proposals add about 281, the light proposal perhaps 60, this one 22. §7 proposes cuts that bring the total back
to about 2,310.

**Swapping the renderer**, in one line per level: replacing how one thing is drawn is a 200-line cliff today and
becomes a `hit` function with the geometry proposal; replacing the look is a `Style` value and is done; replacing the
host is the artefact as files and is done; replacing the renderer wholesale is impossible today, because a drawable
only knows how to emit shaders for this rasteriser, and becomes possible through `forms`. It should be modular at
exactly those four seams and nowhere inside them (§10).

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
hardware rays**. The `forms` seam (§10) is what would let a native loader use them later without the scene changing.

Columns: **works** as the proposals are designed; **small addition** (named); **ruled out**, and what to change. "P" is
the new `pass` (§5). Amendments A1 to A4 are §6.

| Technique | Works as designed | Needs a small addition | Ruled out by the design; the change |
|---|---|---|---|
| **Many lights** (tiled or clustered culling) | A handful of lights, each a term in every lit shader (light sketch). | **A light family** (A2): one function over an index, each member with a reach (a sphere). Then a per-frame map table builds per-tile lists: 40 x 25 tiles of 32 px at 1280x800 is 1,000 tiles; against 256 lights that is 256k sphere tests, one dispatch (*estimate*: under 0.05 ms). The shader loops over its tile's list. | A list of distinct light functions cannot be culled: each is different code, so a tile cannot loop over "its" lights. Families fix it. |
| **Deferred shading** | Not needed: forward shading with per-thing modules is what the materials proposal builds. | The materials proposal already records **the set of channels the style reads** (its step 1). That finite set is a G-buffer layout for that scene and style, so a deferred path is derivable without a fixed channel list. | A thing's own wrap or `scatter` in a deferred lighting pass needs a thing id and a switch over every thing with its own code, or one stencilled full-screen pass per such thing. Cost grows with the number of wrapped things, and deferred defeats 4x MSAA (4x the G-buffer). **Leave deferred alone** (§11); clustered forward gets the many-light benefit. |
| **Shadow maps**, cascades, filtered (PCF, PCSS) | No: no pass, no off-screen target. The style's shadow block traces each opaque pixel against skeleton capsules, which is exact for round chains and blind to every other shape. | **P**: a depth pass from a camera that follows the sun, read with `.project` and `.at` (§3.1). Cascades are three passes. PCF and contact-hardening (PCSS) are expressions over `.at`. A comparison sampler is one descriptor field. | Virtual shadow maps (page tables filled from feedback of the previous frame) need history: never (§11). |
| **Ray-traced shadows** | Yes, approximately: `castRay` against capsules plus the ground; exactly against one field (`march`) or one author tracer (`hit`). | Light `distance` and `size` (A2), so the shadow ray ends at the light and its cone is the light's apparent size. | Exact shadows of every rasterised surface: the geometry proposal says no, because a per-frame hierarchy needs a sort. For Loft's own surfaces it does not (§8.4: the parameter grid is the hierarchy). Offline first. |
| **Area lights, soft shadows** | Soft shadows from the closest-approach cone (`gap / width`) of any tracer: a penumbra in closed form, no noise. | A2's `size` (the cone) and `xi` (a sampled point, for still-mode accumulation or an integrator). Glossy response to an area light by widening the lobe by the light's size (representative point, inherited). | Exact area-light shading of an arbitrary response (linearly transformed cosines need a fitted table per response; an open `scatter` has none). In a still, sampling is exact. |
| **Screen-space reflection** | The `reflection` post pass, for water only. | A post pass can march today; it lacks normals (it reconstructs them from depth) and HDR. With P, a style can render a normals pass if it wants exact ones. | Reflections of what is off screen: inherent to the method. |
| **Planar reflection** | No. | **P** with a mirrored camera (§3.2). Oblique near-plane clipping (inherited, Lengyel 2005) is inside that camera variant. | |
| **Ray-traced reflection** | Sky only (`glossed`), or one field or tracer exactly, or every thing as a blob in its mean colour (`castRay`). | A3: a hit says which thing, copy and body coordinate it met, so the reflection evaluates that thing's real paint there. | Reflections of exact rasterised surfaces in real time: as for shadows. |
| **Ambient occlusion** | Closed-form occlusion of spheres and capsules (a relative named by the geometry proposal); screen-space AO is a post pass today (depth is readable). | Nothing. A bent normal from capsules is an expression. | |
| **Global illumination**, probes | No. | Probes rendered **this frame** are P with six cameras into a small target (a 32² cube per probe, 6 passes). Irradiance from capsules (`castRay` cones) is an expression. | Probes that converge over frames (the common real-time method) need history: never. Probes baked once at build for a static part of a scene: possible as a build-time pass, not proposed. |
| **Global illumination**, traced | No. | A separate renderer over `forms` (§8). | In real time, ruled out by cost on this hardware (§8), not by the design. |
| **Temporal accumulation, denoising** | No, by design: nothing carries over. | **Still mode** (§3.3): many samples of one instant with fixed seeds, averaged into a float target. Spatial denoising (an edge-aware blur) is a post pass over float targets from P. | Reprojection with history (TAA, temporal denoisers, temporal upscaling): never in the scene or the artefact (§4). |
| **Anti-aliasing** | Better than the usual: 4x MSAA plus analytic coverage from `gap / width` for every tracer (an exact edge, no history). | Still mode adds sub-pixel jitter per sample for the stills that want more. | TAA: never; it is not needed. |
| **Physically based surface response** (evaluate and sample a function of in and out directions) | No: `shade(ctx, styled)` answers after the style, with no light direction. | **A1**: `scatter(ctx, toward)` on the paint and `ctx.scatter`. Its default, from `base`, `gloss`, `metal`, is a standard microfacet response that an integrator knows how to sample. | Importance sampling of an author's own `scatter`: an integrator can only sample it by the cosine and weigh it against the lights (multiple importance sampling). Glossy own responses are noisy. A `sample` partner key is a relative, added on first need. |
| **Participating media** | `draw.density`: Gaussian blobs integrated in closed form along the view ray; clouds take point flashes. | A `medium` form (A3) so a second reader sees the density as a function. A light's occlusion through a medium is the same closed-form integral along the shadow ray. | Multiple scattering in real time. |
| **Subsurface scattering** | Stylised: a channel a style reads (wrap lighting, a translucency term from the light's side). | `scatter` can include a smooth wrap term. | True subsurface (a random walk below the surface): offline only. |
| **Thin-film interference** | The beetle in the materials proposal, in three wavelengths. | Written as `scatter` (A1), it answers each light at the right half-vector angle, not only the view angle. | |
| **Diffraction** (gratings, pits, feathers) | The grating equation per wavelength is a closed form in `scatter`: exact, noise-free, per light. Loft is unusually good at this (§9). | | Wave-optical effects across the scene (diffraction around edges, speckle): never. |
| **Dispersion** | Refraction of the sky at three indices of refraction (three closed-form rays) in a wrap or `scatter`. | | A spectral integrator (refraction rays at a sampled wavelength through the scene) needs a wavelength in the shading context: §9 leaves room, adds nothing now. |
| **Caustics** | Closed-form caustic patterns a light can compute (a light that is a function can be the focused pattern under a known swell). | | Caustics found by tracing: hard even for path tracers (needs light tracing or photons); offline only, and not proposed. |

Reading the table by column: **the design rules out only what needs history, and real-time global tracing of
everything**, and the second is a cost, not a door. Every other technique is either already expressible or one named
addition away, and the additions concentrate in two places: the pass (eight rows) and the light's shape (four rows).
