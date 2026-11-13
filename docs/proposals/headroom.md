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
