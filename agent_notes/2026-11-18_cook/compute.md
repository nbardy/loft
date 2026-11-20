## Your lane: a compute escape hatch, run at build, load or per frame (worktree `~/git/_wt/loft-compute`)

Today the only stepped GPU compute an author can write is `recur` (expressions; baked by `src/host/bake.ts`, results
stored in the artefact's static data so any frame draws alone). Frame stages are otherwise fixed: a new kind of
compute needs a library change. The owner wants an escape hatch: **the author writes raw WGSL compute shaders**, and
can run them ahead of time to precompute (bake an SDF or noise into a 3D texture, LUTs, a particle sim) as well as
per frame. Read `src/map/recur.ts`, `src/compile/recur.ts`, `src/host/bake.ts`, `src/artefact.ts`,
`docs/artefact_format.md`, `src/compile/context.ts` (`ctx.table`, raw Drawables), `rust/loft-loader` and the guide's
"Where you can bring your own" section first.

Build one public construct (write the user's code first; cut names) for: author WGSL compute entry points with
declared resources (storage buffers and textures, sizes, formats), dispatch sizes, and **when it runs**, a sum type:
- **build**: run once at `loft build` on the GPU, results written into the artefact (like a baked recurrence), so
  loaders never run it; artefact size stated.
- **load**: run once when the artefact is loaded (the Bun/Dawn host, `loft serve`, and the Rust loader), for results
  too big to ship; must be deterministic.
- **frame**: run every frame as a pure function of (time, inputs, params, other resources), never of the previous
  frame (frames stay pure); stateful sims go through **steps**:
- **steps**: a state stepped N times at a rate and stored per kept step, read as a function of time (generalising
  what `recur` does, with raw WGSL steps). Reuse the recur bake path rather than writing a second one; say whether
  `recur` should become a use of it.
Outputs are readable from expressions (`fetch`/texture sample nodes) and from raw drawables and raw compute, and passes
are ordered by what they read. Checks and `solve` cannot read raw WGSL: they answer `undecided: raw compute` by name
(see `src/ranged.ts`). Validate resource declarations at the boundary with typed errors; bound sizes.
The confined training compile (GPU-less child) only emits; the server/loader runs the compute. The Rust loader must
run `load`/`frame`/`steps` too (cargo build, run `loft-view` on your example).
Examples: (1) bake a noise or SDF volume into a 3D texture at **build** and raymarch it in a raw or expression
drawable; (2) a raw-WGSL particle sim under **steps** (e.g. dust settling around a box obstacle) drawn as points by a
raw drawable; (3) a **frame** pass making a small texture an expression reads. Report times: bake ms, load ms, per-frame
ms at 1280x720, artefact bytes. Docs: guide section + `docs/artefact_format.md` (bump the format version if needed,
keep readers of older versions working only if persisted artefacts exist; say which).
Another worker (`feat/spray`, a generic points drawable) and (`feat/volume`, raymarched density) run in parallel:
do not build those; your examples use raw drawables.
