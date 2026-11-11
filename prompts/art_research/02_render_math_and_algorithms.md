# Art research 2: the mathematics and algorithms of drawing it

A handoff prompt, written 2026-11-11. It poses a problem, says what is known and how well, and asks for mathematics
and algorithms. The framing is ours and may be wrong; replacing it with a better one is a good outcome.

## The situation

AI agents author stylised 3D art for a real-time game: a sea, ships, weather, large animated creatures. Players run
it through WebGPU, often on integrated graphics, with many moving things on screen. The companion prompt,
`01_art_tool_from_first_principles.md`, covers the authoring side. This one is about getting shapes onto the screen.

## The problem

There are four established ways to draw a shape. Each has its own literature and usually its own engine.

| Way | Good at | Pays for |
|---|---|---|
| Triangles from a formula | What the hardware is built for; forward warps and motion are free | Blends between parts; anything that cannot be listed |
| Exact intersection of listed simple shapes | Exact edges, one solve per shape | Needs a closed formula per shape; no blends |
| Marching a function along rays | Any composition: blends, folds, warps, fractals; nearness for soft effects | Steps times the cost of one evaluation, on every ray |
| A table computed once | Cheap to read | Fixed; has to be rebuilt when the thing changes |

We think the problem is not a better engine of one kind. It is making them interchangeable:

> An artist states what a thing is and how it moves, once. Each statement can be read several ways: as triangles, as
> listed shapes, as a marched function, as a table. Find the language in which every statement has as many readings as
> possible, the conditions under which two readings give the same picture, and a bound on the difference when they do
> not. Then choose the reading per part from costs that can be counted before drawing.

If you think this is the wrong problem, say what the right one is and why.

## What we want

1. **A map.** For each thing an artist wants to say and each way of drawing it: the exact reading with its
   conditions and cost, or a reason there is none.
2. **The gaps filled.** The ones we know of:
   - smooth blends between separate parts, drawn as triangles;
   - a rule for splitting displacement between vertices and shading;
   - soft volumes (cloud, glow, spray) at a fraction of their present cost;
   - motion that does not need per-frame work on the CPU;
   - outlines that look the same whichever way a thing is drawn;
   - soft shadows and other nearness effects for things that are listed, not marched.
3. **Certificates.** Numbers computed from the description that prove a reading is safe, so that an agent gets yes
   or no without rendering.
4. **A cost model** that picks the reading, with the picture guaranteed within a stated pixel tolerance.
5. **Something better.** A representation or an algorithm under which some of these distinctions disappear. This is
   the part we want most, and the part we are least able to specify.

## Budgets

- One object under 1 ms of GPU time at 1280x800 on integrated graphics. This is the owner's bar: "single objects need
  to be sub 1 ms if we're going to render a lot of them".
- CPU time per object well under that.
- Dozens to hundreds of objects on screen.
- An edit reaches the screen in about a second, so anything computed once has to be computed fast. Caching the
  coral's distances took 1 to 7 seconds, depending on the machine's load, against 0.03 seconds to list it.

## Where it stands

Measured on 2026-11-11 on an M-series laptop, GPU time of the draw alone unless stated.

| What the artist wants to say | Cheapest reading found | Status |
|---|---|---|
| A body around a skeleton | Triangles from the formula | Built; a whole serpent about 0.15 ms |
| Repeats, branches, folds | List the copies, intersect each exactly | Built; coral 1.4 ms against 4.5 ms cached and 12.6 ms marched |
| A smooth blend between parts | Triangles, each vertex pushed onto the blend | A 2D test of two convex parts only |
| Twist, bend | Move the vertices; normal from the warp's Jacobian | Published in 1984 |
| Noise displacement | Vertices for coarse, shading for fine | Published in pieces; no rule for the split found |
| Cloud, glow | A closed-form integral per pixel | Built; 9 to 10 ms |
| Motion | A pose computed on the CPU each frame | Built; 5.4 ms a frame, the largest cost |
| Fractals, endless repeats | March | Built; no cheaper reading known |

## Candidate results

Ours. Test them, extend them or break them. Each is labelled with how far it was checked.

- **Unfold.** A fold made of shifts, turns, mirrors and uniform scales has two readings from one description: asked
  at a point, or listed as copies. They are the same solid where no copy crosses a mirror. Measured, with tests.
- **Push to blend.** Each part keeps its own triangles. Each vertex moves along its own normal to the first zero of
  the smooth union of the parts' distance functions. The parts still overlap inside and the depth buffer takes the
  union. Derived for two convex parts with exact distances; measured in 2D only.
- **Reach.** For that construction a push limit of a quarter of the blend radius is enough on paper and leaves an
  error at the seam that does not shrink. Half the radius gives second-order convergence. Found by measurement
  after the derivation said otherwise.
- **Blend load.** A guess: with several neighbours, the construction covers the blended surface when the sum of
  their blend weights, each scaled by how much it faces the part, is at most 1. Not tested.
- **Local blends.** The usual smooth minimum, nested pair by pair, is not exactly local and depends on the order of
  the parts: in a six-part example a part three blend radii away changed the result by 0.0001 radii. A symmetric
  form with compact support is exactly local. Measured.
- **March inside each part's hull.** With an exactly local blend, marching only a part and its neighbours inside
  that part's own hull, and letting the depth buffer choose, gives the true surface. Derived; not built.
- **Solve once per vertex.** A blended body with a skeleton can be found along rays from the skeleton by Newton's
  method started from the neighbouring vertex: 2.8 evaluations per vertex in a 2D test.
- **Bend bound.** A distance evaluated in a swept body's own coordinates is a safe step in the world after scaling
  by one minus curvature times radius. Derived; textbook geometry.
- **Two more readings of an expression.** A slope bound and an interval bound, computed from the same expression
  as the value, give hulls, pruning and safe steps. Derived from published work; not built.

## Literature

The fuller notes, with how each source was checked, are in `docs/tracing_sota.md`. Read it as a starting list, not
as a survey.

- Marching, per-region pruning: Keeter 2020 and 2025; Zanni 2023; Barbier and others, Lipschitz Pruning, 2025.
- Marching, steps: Keinert 2014; Galin and others, segment tracing, 2020; Seyb and others, non-linear sphere
  tracing, 2019.
- Triangles: Barr 1984 (warps); Blinn 1978, Niessner and Loop 2013, Olano and Baker 2010 (displacement and its
  filtering); Vaillant and others, Implicit Skinning, 2013 and 2014 (vertices moved onto a blended field).
- Many triangles: Nanite 2021; CuRast 2026, whose abstract says ordinary rasterising is still ten times faster
  for low-poly meshes.
- Splats: Kerbl and others 2023 and the speed papers since.
- Not read by us, and likely to bear on blends: Hoffmann and Hopcroft's potential method (1985 to 1987), Rockwood's
  displacement method (1989), Middleditch and Sears (1985). Read these before claiming anything new about blend
  surfaces.

## How to work

- **Label every statement**: textbook, literature (cite it, and say whether you read the primary source),
  derivation, conjecture, or measured.
- **Test a derivation numerically before you believe it.** Two of ours were wrong until tested: the reach above,
  and a proof that assumed the nested smooth minimum was local.
- **Look for counterexamples** before looking for proofs: three parts at a point, parts that are not convex,
  distances that are only bounds.
- **Measure GPU time with timestamp queries** around the draw, with the variants interleaved in the same rounds
  and the best of many taken. Whole-frame times on a busy machine moved by 1.5 ms between identical runs.
- **Count the CPU.** It was the largest cost and we found it late.
- **Compute your pictures and look at them.**
- **A negative result is a result.** Say what failed and what it rules out.

## What to deliver

1. Results ranked by status: proved, measured, conjectured, refuted. Each with the test that put it there.
2. The tests and figures as runnable code.
3. A recommendation: what to build first, what it should cost, and what would show it was the wrong choice.

## Pointers

- Notes and sources: `~/git/loft/docs/tracing_sota.md`; pictures: `~/git/loft/docs/marching/index.html`.
- The figure and count tools: `~/git/loft/tools/explain_marching*.ts`, `tools/explain_blend.ts`.
- GPU timing: `bun src/cli/main.ts profile <scene>` and `tools/tracer_bench.ts` in `~/git/loft`.
- The two readings of a fold: `~/git/loft/src/lib/similar.ts`, `src/draw/copies.ts`, `examples/coral_list.ts`.
