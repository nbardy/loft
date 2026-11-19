# Loft: core

What Loft is for, what we want from it, and how we judge a change. The stance behind it is `docs/PHILOSOPHY.md`;
the design goals are `docs/design/render.md` and `docs/design/motion.md`. The v1.0 specification is in `docs/spec/`.
When a design and this page disagree, change the design or argue here.

## The aim

The best 3D rendering library a model (or a person) can write for: pictures that look state-of-the-art, drawn very
fast on the GPU, from short code that says exactly what is meant, with nothing the author cannot reach.

The product is the picture. The reward we train on is aesthetic; checks and `solve` are authoring tools.

## Priorities, in order

1. **The picture.** State-of-the-art looks: lighting, materials, motion, camera.
2. **GPU frame time.** Speed is judged on the GPU: frame time, then pipeline compile and load time. CPU build time is
   a cap (2 s for training under the Rust compiler), not a goal; it gets work only when the cap binds.
3. **Tokens.** The author writes the least code that says the thing, in the lean `.loft` surface (`docs/spec/LOFT_V1_SYNTAX.md`): infix maths, inferred types, WGSL only as the escape.
4. **Truth.** Nothing Loft reports may be false: a check, a solve, a verdict or an error. What cannot be decided says
   so by name.
5. **Lean.** Little code, few types, one path. Every name buys a capability the author did not have.

## What we want

### Drawing

- **Analytic intersection** wherever the maths solves it: impostors (sphere, capsule, cone), planes and the sea, and
  more closed forms (quadrics, boxes, tori) before anything iterative.
- **Raymarching**: sphere-traced fields inside a bound, folding and domain repetition (a `Warp` gives the cell id),
  relief on surfaces, hit refinement, normals from the field's gradient.
- **Raster analytic surfaces**: sweep, loft, lathe, extrude, thicken, evaluated in the vertex stage.
- **Instancing**: copies, posed and points, with culling and levels of detail. Many parts of one field use a spatial
  lookup (cells or bounds), never an N-way `min` at every step.
- **Media and particles**: marched and Gaussian volumes with one transmittance; specks for dust, spray and rain.
- **Physically based shading** at the state of the art: microfacet BRDF with energy compensation, image-based and sky
  light, a moving sun, good shadows and AO, subsurface where it matters. Audit against the state of the art and close
  the gaps; do not assume we are there.
- **Camera and post**: depth of field, film grain, a pass composer, multi-pass and feedback.

### Speed

- **Frame budgets**: training draws 512² × 16 frames at ≤ 50 ms GPU a frame; viewing is judged by p95 frame time.
- **Emit decides per site by GPU cost**: inline a small or once-called function, unroll a constant loop of ≤ ~8 small
  bodies, otherwise a call or a loop over a table, and a spatial lookup for many instances. The recorded graph stays
  compact either way; unrolling is an emit choice, never a recording.
- **Fewer reads, fewer registers**: packed tables, folded constants, small shaders, a pipeline cache.
- **Load time does not grow with content**: shaders are shared per kind of thing, with per-instance data in tables.
  Five crabs need a handful of shader modules, not 305 (measured 55 s to load on 19 Nov). Score shader module count
  and load time beside frame time.
- **Precision**: f32. f16 stays frozen until a scene shows a frame-time gain.

### Engine

- **One path, in Rust on wgpu**: build, serve, tools, the wasm viewer and Keel share it, on Metal, Vulkan, DX12 and
  WebGPU. The Rust compiler parses `.loft` directly; there is no TypeScript in the product.
- **A compact graph**: a graph function is one emitted WGSL function and its uses are calls; a repeat is a loop; instances and
  stations are tables; sibling subgraphs that differ only in constants are recorded once with a table of the
  constants. Graph size follows distinct code, not the number of uses.
- **Few types**: one kernel, one ray template, one medium, one verdict, one error, one format, one identity, one
  operator table. Kinds are sum types with one exhaustive dispatcher; inputs are classified once at the boundary; a
  missing case is a typed refusal that names the fix, never a fallback.
- **A small core, content outside it**: sea, sun and style are library values a scene may omit, not part of the core.
  Assets come in (meshes, textures, motion clips) and picking and readback go out.
- **Embeddable**: a Loft subject draws on a device someone else owns (Keel and room-runners), with wgpu and naga
  pinned to the host's versions.

### Authoring

- **One lean surface, `.loft` v1.0** (E+, chosen by a blind test and the research scientist): infix maths, inferred
  types, `build`/`emit` staging, `wgsl(...) -> T { ... }` blocks as the raw escape; a lossless s-expression form of the
  IR for agents; a `.lark` grammar for constrained decoding.
- **One escape ladder**: graph function, raw function, `draw.shader`, compute. Each rung can call the ones below it.
- **Compute at every time**: build, load, frame and steps. Any maths, any algorithm.
- **Nothing hidden**: `loft show` prints the generated WGSL; errors quote the author's line.
- **Swappable parts**: looks, materials and motions are values you replace. Libraries save tokens and never fence the
  author in.
- **Docs that cannot drift**: `loft context` is generated from the code; changes ship with a migration note and a
  codemod.

### Truth

- **Checks and `solve` never say something false**. A build solves and reports holds as `Unproved`; `loft check`
  proves; a margin finer than the arithmetic is `Undecided` by name.
- **Picture parity is the gate** for engine changes. Use the approved v1.0 pictures and the tolerance in the
  Phase 1 picture gate (`tools/dev.sh run loft-cli -- gate`).
- **One identity record** for a picture: source, precision, parameters, inputs, format.

## How a change is judged

- **Theory first, to the number.** Predict what the run will show (frame ms, node count, parity) in the lane note
  before building; run once to score it. A miss fixes the theory.
- **Score by the priorities above**, in order: picture parity, then frame time, then tokens, then lines.
- **One full suite per wave**, run by the integrator; workers run focused checks. The machine is shared.

## Not goals

- CPU compile speed for its own sake.
- Back-compat for code. Persisted data (bundles, saved scenes) gets one reader or a migration; code changes with its
  callers.
- Two engines kept alive. Alternatives live on branches; the default is the only path once parity passes.
- Convenience names, mirrors of internals, or options that buy no capability.

## Where we are (19 Nov 2026)

The root Rust workspace runs the Phase 1 rendering slice. Capsules, blobs and the hermit crab exercise the five gates
at one and twenty instances. See the [Phase 1 results](agent_notes/2026-11-20_meta/phase1.md) and the
[v1.0 build order](docs/spec/05_NUMBERS_AND_BUILD_ORDER.md). The retired engines and their measurements are recorded
in [DEPRECATED.md](DEPRECATED.md) and [docs/legacy/](docs/legacy/).
