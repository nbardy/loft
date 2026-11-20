# The five proposals, reconciled

Written 2026-11-13. Five design proposals for what an author can write in Loft, each by a separate worker, none
built. This page says what they add up to, where they disagree and how that is settled, what it costs, and what the
owner has to decide. Each proposal has its own scenes, types, predictions, staged plan and stop rule.

| Proposal | Adds | New exports | Words in `loft context` | Days |
|---|---|---|---|---|
| [`light.md`](light.md) | a light is a function of a world point; `scene({ lights })`; `lamp`; the sun gains a strength | net +1 | +63 | 5.5 |
| [`materials.md`](materials.md) | paint returns a colour plus named channels; a thing's own `shade`; `glossed` reflects the sky | 2 | +105 | 5 |
| [`geometry_and_rays.md`](geometry_and_rays.md) | `draw.mesh` (triangles made by code), `draw.traced` (your own tracer), `march`, `castRay` | 6 | +91 | 9 |
| [`state.md`](state.md) | `recur`: a step function baked on the GPU into the scene's files | 1 | +85 | 10 |
| [`headroom.md`](headroom.md) | `pass` (a picture drawn off screen), a still mode that averages samples; a review of the other four | 2 | about +60 | 11 |
| **All five** | | **12** | **about +400, to about 2,650** | **about 40** |

The days are the proposals' own estimates of work, not of elapsed time.

## What the set does

An author can then: light a scene with any light they can write as a function; say how a surface answers light
without choosing from a menu of materials; draw triangles their own algorithm made and write their own tracer in one
function; cast a shadow or reflection ray; step a simulation and still draw any frame alone; render a thing from a
second camera into a texture (a shadow map, a mirror); and ask for a still with soft shadows, depth of field and
exact motion blur. Every existing scene stays byte-identical in all five, with two stated exceptions (below).

## Where they disagree, and the ruling

The headroom review was written before the light proposal existed, so its amendments to light were made against a
sketch. Reconciled here:

| Question | Light proposal | Headroom review | Ruling |
|---|---|---|---|
| What a light returns | `{ toward, amount, far? }` | `{ toward, colour, distance, size }` | `{ toward, amount, distance?, size? }`: `amount` is a colour, linear, and may exceed 1; `distance` is the light's own name for `far`; `size` (how wide the source looks from the point) is optional and only a still or a soft shadow reads it. |
| Sampling an emitter with extent | room left, not designed | an optional second argument `xi` | Take the optional argument. A plain author never writes it; a helper for an area light does. |
| Skipping lights | an optional `bound` a helper attaches | a family of lights over an index, each with a reach | `bound` now. The family waits for a scene with more than the cap of seven. |
| How a style gets the lights | an array on the shading context, the sun first | `ctx.lights(term)`: a sum the style asks for | The fold, with the combiner stated (`sum` or `largest`, since the lighting recipe wants the largest), so the compiler stays free to loop, cull or defer. |
| Things that glow | they do not provide a light: one constant handed to both the glow and the lamp | (followed the earlier sketch: glows provide one) | The light proposal's: nothing is coupled behind the author's back. |
| A thing's own answer to light | `shade(ctx, styled)`: a function of the style's finished colour | add `scatter(ctx, toward)`: a function of the light's direction | Both. `scatter` is what a stylised look calls once per light and an integrator once per sample; `shade` stays for changes to the finished picture. `scatter` goes in the first build of materials. |
| What a ray returns | distance, softness, an average colour | also the thing's id and its place along its chain | Add both. |
| What a second reader of a scene can reach | (not addressed) | `Drawable.surfaces` becomes `Drawable.forms`: a surface, a mesh, a field, a medium, or the author's own `hit` | Adopt. This is the one seam that makes the renderer replaceable; baking, the CPU checks, a physical mode and differentiation all read it. |
| Rays against ordinary surfaces | ruled out (no structure to search) | a surface's own (u, v) grid is already a hierarchy that can be refitted without sorting | Unproven. Keep it out of the build; measure it once as an experiment before anyone relies on it. |
| State and passes | (not addressed) | a step may never read a pass | Adopt: otherwise baked state becomes frame history by the back door. |

Two existing pictures change, once each: the four storm scenes when `flash` becomes `lamp` (the bolt then also lights
bodies and the sea), and any picture whose traced solids were being missed by the screen-space outline, if that
suspected fault is real (the geometry proposal found it and did not test it).

## What stays out, by decision of all five

File import of meshes. A path tracer inside the real-time renderer, or as a style: physically based light is a separate
offline tool that reads the same scene through `forms`. Anything carried from one frame to the next, in a scene, a
file, a check or a clip. Live state that a game's inputs change (refused in the first version of `recur`).

## The context budget does not survive as it is

`loft context` is 2,251 words. The five add about 400. The headroom review's plan: move the domain modules (weather,
sea foam, throwing) out of the API map, where they are subject matter and not mechanism, and tighten each proposal's
own lines; that lands at about 2,325.

## Order of building

1. **Light and materials together** (they change the same shading context): about 10 days. First picture: a lantern
   lighting its floor at night.
2. **Geometry and rays, with `forms`**: about 9 days. Gives light its shadow rays and materials its reflection rays.
3. **`pass` and the still mode**: about 11 days.
4. **`recur`**: about 10 days. Independent of the rest; can run alongside any of them.
5. `loft context` regenerated once, at the end, and the docs' snippets re-run.

Each stage keeps the gate used all week: existing pictures byte-identical, every checker green, predictions scored.

## What the owner has to decide

Everything else follows each proposal's own recommended answer unless he says otherwise.

1. **Before the first training run, or after?** All five change what an author writes, so they change what a model
   trained on Loft has learned. Either the run waits for them, or it runs on today's library and these land as the
   next version with a regenerated context.
2. **The context budget**: cut the domain modules from the API map to stay near 2,300 words (recommended), or let it
   grow to about 2,650.
3. **Glows do not light things by themselves**: the author hands one position to both the glow and the lamp
   (recommended), or a glow lights automatically.
4. **`flash` is removed** and four storm pictures change once (recommended), or it is kept beside `lamp`.
5. **A scene with baked state needs a GPU to be checked** (recommended: a CPU bake of a chaotic step measures a
   different run), or checks stay CPU-only and such scenes are approximated.
6. **Physically based light only as a separate offline tool; frame history never inside Loft** (recommended by all
   five), or one of those lines moves.
7. **Which stage first**, if not the order above.
