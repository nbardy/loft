# Loft: API design (v0)

Loft is a way of describing animated 3D things as code, and a compiler that turns the description into shader source
plus tables of numbers that any WebGPU host can draw. The reasoning behind it is in
`room-runners-arena-lib`, `notes/design/2026-11-10_creature_pipeline_options.md`, sections 10 to 12. This file is the
contract for building it.

## 1. What is being built

```
 authoring code (TypeScript)          the compiler (TypeScript)             a host
 builds a scene description   --->    scene -> Artefact              --->   draws an Artefact
 src/map, src/draw, src/geom, ...     src/compile                           src/host (WebGPU: browser, bun headless)
                                      Artefact = WGSL + tables + draw list  later: Rust/wgpu inside the game
```

Nobody compiles TypeScript to anything. TypeScript runs normally and *builds a graph of expressions*; the compiler
turns that graph into WGSL (for the GPU) and JavaScript (for the once-per-frame CPU pose pass). The game would load
an Artefact; it never runs authoring code.

Five ideas, in the order the code depends on them:

1. **Expressions** (`src/expr.ts`, done). Authoring code builds `Expr` values. One DAG, two backends (JS, WGSL), tested
   to agree, with `fetch` (an entry of a per-frame table) and `iterate` (a real loop) as well as arithmetic.
2. **Curves are the atoms.** A curve is any function of one variable, `(x: Expr<f32>) => Expr<T>`. Curves built from
   keys are stored as rows of a table, so editing one is a buffer write, not a recompile. Named numbers are `param`s
   with the same property.
3. **A body is a coordinate system.** A strand (a spine plus a thickness) defines coordinates `(m, v, rho)`: `m` in
   [0, 1] along the spine, `v` in [0, 1] around it, `rho` the distance out as a fraction of the thickness. The skin is
   `rho = 1`. Paint, attachments, outlines and foam are all expressed in those coordinates. A coordinate system is a
   `Chart`; charts compose, which is the scene graph.
4. **Time is an argument, not a system.** Every expression may read `time` and host inputs. A body at a chosen time
   is a *slice*; the pose pass can evaluate the same body at several times (now, an attack's arming time, recent past)
   to derive telegraphs, smears and wakes.
5. **Description and schedule are separate.** The scene says what things are. The compiler decides where each part is
   evaluated: per frame on the CPU (spine stations), per vertex (surfaces), per pixel (fields on the sea, volumes).

Open types: a shape is a function whose signature says how it can be drawn (section 7); an object may be several
(a body is a Surface, a Chart and a Skeleton). The one rule: everything is pure.

## 2. Authoring API

Everything is built on the map algebra of section 7: shapes are functions over `Expr`, drawn by one generic drawable
per signature. The names below are the library on top of it.

```ts
import { crest, draw, keys, on, palette, param, place, points, scene, sea, strand, through, toon, vec3, wave } from "loft";

const girth = keys([[0, 0.3], [0.06, 0.9], [0.45, 1.0], [0.85, 0.4], [1, 0.02]]);   // a table-backed curve
const body = strand({                                  // a sweep (a Surface, a Chart, a Skeleton) that draws itself
  name: "serpent",
  spine: (m) => through(m, [[-9, 0.4, 0], [-4, 0.9, 1.5], [0, 0.6, -1.2], [4, 1.8, 0.4], [7, 3.2, 0]])
                  .add(wave(m, { axis: [0, 0, 1], waves: 1.6, rate: 0.5 })),
  stations: 40,
  girth: (m) => girth(m).x.mul(param("serpent.girth", 1.1)),
  section: { aspect: 0.9, squareness: 2.3 },
  paint: (m, v) => palette.countershade(v, { back: [0.16, 0.13, 0.33], belly: [0.86, 0.82, 0.62] }),
});
const head = place(strand(headSpec), on(body, [0, 0.25, 0]));     // headSpec's spine is in the neck's frame there
const spikes = draw.points("spikes", place(points(7, (i) => ({ at: vec3(i.mul(0.03).add(0.6), 0.25, 0.9), scale: 1 })), body), spike);
                                                                      // spike: another strand, drawn once per point
const fin = crest({ on: body, name: "fin", from: 0.12, to: 0.8, height: keys([[0, 0], [0.3, 1.2], [1, 0]]), paint: [0.75, 0.2, 0.28] });

export default scene({
  sea: sea({ swell: [{ direction: [1, 0.3], length: 14, height: 0.25, speed: 1.1 }] }),
  things: [body, head, spikes, fin],
  style: toon({ bands: 3, outline: 0.045 }),
  sun: [-0.6, 0.5, -0.6],
  cameras: { three: { kind: "perspective", eye: [-14, 9, -13], target: [0, 1, 0], fovY: 0.6 } },
});
```

`compile(scene) -> Artefact`; `compileScene(scene) -> { artefact, frame(state), cpu(roots), table(name) }` for checks,
tests and the baker. A scene module's default export is a `Scene`. Working examples: `examples/serpent.ts`,
`examples/shell.ts` (a surface written as a map, painted through a fold), `examples/serpent_scales.ts`.

### 2.1 Curves (`src/curve.ts`)

- `type Curve<T> = (x: In<"f32">) => Expr<T>`; the map signature `Curve` is `Curve<"vec3">`.
- `keys(points, { interp?: "smooth" | "linear" })`: keyed curve sampled into a curve row (`CURVE_N` vec4s), so an edit
  is a buffer write. `ramp(colours)`. `through(m, points)`: closed-form Catmull-Rom through points that may move.
- Motion, all closed form in time: `wave`, `retarded(t, m, speed)`, `response(t, { f, zeta, r })`, `pulse`.
- `keys` makes a new row per call: build a curve once, outside the functions that use it (the compiler refuses a row
  first seen after layout, which is how that mistake shows).

### 2.2 The library (`src/geom/`)

- `strand(spec) -> Strand`: `sweep` with defaults and a look. Spec: `name`, `spine(m)`, `stations`, `girth(m)`,
  `section` (`aspect(m)`, `squareness`), `twist(m)`, `tips` (`"round" | "point" | "open"` or with lengths), `up`,
  `level`, `paint(m, v)`, `detail` (rings along, segments around). A `Strand` is a `Body` and a `Drawable`; placing it
  (`place(s, chart)`) or slicing it (`s.at(t)`) keeps its look; `draw.points(name, pts, s)` instances it.
  Body coordinates: m in [0, 1] along the spine, v in [0, 1] round it (0.25 the dorsal line, 0.75 the belly), rho out
  as a fraction of the thickness (skin at 1). Frames are rotation-minimising (Wang et al. 2008) with optional levelling;
  radii scale by 1 / sqrt(stretch) (volume preservation).
- `crest(spec) -> SurfaceDrawable`: a `sheet` standing on the line of constant v of a body, rising along its section's
  outward direction, raked and fluttering; drawn two-sided with ink on its free edge.
- `sea(spec) -> Sea`: the root chart (x, z, height above the water), and the water as a glazed surface in that chart:
  a camera-following grid, the style's `water`, foam where any opaque body meets it (from scene depth).

### 2.3 Style (`src/style.ts`)

A style is a program in the same language: `{ shade(ctx), ink(ctx), outline, water(ctx), shadow(ctx), sky(ctx), post,
clear }`. `shade` lights a base colour (body coordinates are not in its context: paint has already used them) and tints
it under water. v0: `toon`.

### 2.4 Planned (unchanged in intent; to be written on section 7)

- Space-time: `telegraph(obj, { arm })`, `smear`, `wake` from `body.at(t)` slices.
- Soft things: `decal` (a glazed surface laid on the sea), `cloud` (`draw.density`), `glow` (`draw.curve`).
- Checks evaluated on the CPU through `compileScene(...).cpu`: `clearance`, `minScreenSize`, `budget`.
- Local form and sculpt layers: stamps and displacement tables over (m, v) (`tabulate` of a function of (m, v)), the
  mesh round trip (`bake` out with body coordinates as UVs; fit back as a radius function plus a sculpt layer).

## 3. The Artefact (`src/artefact.ts`, done)

Read that file; it is the host contract. In short: a fixed prelude (bind group 0: frame, inputs, params, curves), an
artefact-declared bind group 1 of per-frame tables, complete shader modules, a list of buffer-less draws, optional
post passes, and `pose(state)` which fills the per-frame tables on the CPU. `FrameState.curves` is the curve table the
host holds now, so a `setCurves` edit moves the CPU pose exactly as it moves the GPU.

Per-frame tables are `TableSource`s (`src/expr.ts`): `entries` entries of `lanes` vec4s, a `role`, and `prepare(p)`,
which the compiler calls once (with `p.compile(roots)` for CPU expressions) to get the fill. The compiler finds them by
walking the expressions it emits (`fetch`), or a raw-WGSL drawable declares one with `ctx.table`. They are packed, in
declaration order, into as few storage buffers as hold them (`loft_tables_0`, ...), because a shader stage may bind 8
storage buffers and group 0 uses 3; `fetch(t, i, lane)` reads `t_<name>(u32(i) * lanes + lane)`. More than 5 buffers'
worth fails at compile time with `TableBudgetError`. A fill may read other tables (they fill on first read; a cycle
throws). The tables the library declares:

| Table | Declared by | Per entry | Contents |
|---|---|---|---|
| `body_<name>` (`_<k>` per extra time slice) | every sweep that is drawn or read | 3 vec4 | `p` (position, arc fraction), `q` (frame quaternion), `r` (half-width, half-height, stretch, 0) |
| `shadows` | `src/draw/shadows.ts`, from every drawable's skeletons | header + 3 vec4 | capsules projected along the sun onto the sea: ends, radii, strength, softness, chain links |
| any name | `tabulate(name, f, n, "frame" \| "static")` | 1 vec4 | samples of f |

`stats.stations` counts the entries of tables whose role is `"stations"`.

Foam where the water meets a body is not a table: the sea reads the opaque scene's depth and foams where an opaque
surface lies within the foam width in 3D. It needs no per-kind code and follows the drawn surface exactly.

## 4. Drawables (`src/compile/context.ts`)

```ts
interface Drawable {
  readonly name: string;
  draws(ctx: CompileContext): Emitted;          // { shaders: { key: { code, libs } }, draws: DrawSpec[] }
  readonly skeletons: readonly Skeleton[];      // what it casts shadows with (empty for none)
  readonly surfaces: readonly Bakeable[];       // what it draws that the baker can evaluate (empty for none)
}
interface CompileContext {
  readonly slots: Slots;                        // emit expressions: emitShaderFn(ctx.slots, { name, args, out, roots, ret })
  readonly style: Style; readonly sea: Sea; readonly sun: V3; readonly things: readonly Drawable[];
  table(t: TableSource): TableRef;              // only for raw WGSL; fetched tables are found by themselves
  post(pass: { label; wgsl }): void;
}
type Phase = "background" | "opaque" | "water" | "on-water" | "transparent";
```

- Every library drawable is a `SurfaceDrawable` (`draw.surface`, `draw.glazed`, `draw.curve`, `draw.points`) except the
  sky and the shadows, which are raw WGSL. Nothing in `src/compile` knows a kind of thing.
- A shader module lists the library functions it calls; the compiler puts each either in `sceneWgsl` (when the shared
  functions use it too) or in that module. This is what lets a function that reads scene depth (the sea's contact foam)
  be an ordinary GPU-only library function.
- Order is stated, not wired: a draw names its phase; within a phase, the order drawables emit.
- The CPU backend (`compileJs`) is scalarised and folds constants as it emits; a fetch at a constant index is a direct
  array read, so the pose pass is plain arithmetic on what varies.

A new kind of thing is one module: build a map and hand it to a generic drawable, or write a `Drawable` with raw WGSL
that declares its table with `ctx.table` (see `src/draw/shadows.ts`, which reads every skeleton's joints through the
same expressions the vertex stage uses, and fills its table on the CPU).

## 5. Milestones and ownership

**M1, vertical slice.** One scene (the example in section 2: strand, crest, sea, toon style with an ink outline)
compiles, renders headless to a four-view sheet and a clip, and runs live in a browser with sliders.

| Part | Directory | Owner |
|---|---|---|
| Expressions, artefact contract | `src/expr.ts`, `src/artefact.ts` | done |
| Curves, strand, crest, sea, pose pass, toon style, `compile` | `src/curve.ts`, `src/geom/`, `src/scene.ts`, `src/style.ts`, `src/compile/` | compiler agent |
| WebGPU host, camera maths, capture | `src/host/` | host agent |
| CLI (render, clip, check), contact sheets, video, dev server and browser viewer | `src/cli/`, `viewer/` | tooling agent |

**M2, breadth** (after M1 works end to end): fan, shell, scatter, level of detail; whirlpool, placement, attachment;
telegraph, smear, wake and checks; clouds and glow; more styles.

**M3, pictures.** Scenes that are worth looking at.

## 6. Rules for code in this repository

- Pure functions; no hidden state; deterministic output. No clocks, no randomness except hashes of indices.
- Sum types and exhaustive `switch`; no silent fallbacks: an unknown case throws with a message that names it.
- Tests earn their place by catching a real bug: numerical agreement (CPU against GPU), round trips, invariants.
  No tests that restate a type.
- Comments say why. A non-obvious fix gets a comment at the site.
- f32 tolerances are 1e-4 relative or looser.
- bun runs everything; pnpm installs. `bun test`, `pnpm exec tsc --noEmit`.
- Known Dawn/bun-webgpu facts: `getCompilationInfo` is not implemented (validate with `pushErrorScope("validation")`
  and pipeline creation); `queue.writeBuffer` calls are ordered with the queue, not with encoder commands, so a
  buffer rewritten between passes of one submit holds only its last value.

## 7. The rethink: everything is a map (v0.2)

Loft used to describe kinds of thing (strand, crest, sea, decal, cloud), each with its own hand-written shader
template. Extending it to folding, to arbitrary analytic surfaces, to points and to traced solids kind by kind would
multiply those templates. This section replaces the kinds with one small algebra (built: 7.5; corrections: 7.6).
Section 2 is the library of *names* built on it.

### 7.1 A shape is a function, and its signature says how it can be drawn

| Signature | Name | Drawn by | Examples |
|---|---|---|---|
| index -> frame | `Points` | instancing another drawable, or splats | teeth, spikes, rain, a fleet |
| (u) -> point | `Curve` | a stroke: a ribbon or tube with a width function | ropes, lightning, ink lines, trails |
| (u, v) -> point | `Surface` | a grid evaluated in the vertex stage | bodies, sails, fins, shells, the sea |
| (u, v, w) -> point | `Chart` | its boundary as a surface; things placed inside it | the space around a body; the sea; a whirlpool |
| point -> distance | `Field` | a tracer inside a bound (`docs/folding_and_tracing.md`) | folded and recursive solids |
| point -> density | `Density` | integration along the view ray | cloud, fog |
| coordinates -> colour | `Paint` | the fragment stage | bands, scales, markers on the water |

All of them are TypeScript closures over `Expr`, so they are pure, may read `time` and inputs, and compile to both
backends. An object may be several at once (a body is a `Surface`, a `Chart` and a `Skeleton`).

### 7.2 Three operators

- **Compose in front (a domain map, a `Warp`).** `warp(shape, F)` evaluates the shape at `F(x)`. On a `Field` or a
  `Paint` this is folding: mirror, repeat, polar repeat, spiral, kaleidoscope. On a `Surface` or `Curve` it
  reparametrises (for example concentrating samples toward a tip). On `Points` it maps the index (a generator for
  instances). A `Warp` returns `{ x, id, stretch }`: the mapped coordinate, the cell it came from, and the local
  change of length, so distances stay distances.
- **Compose behind (a range map, a `Chart`).** `place(shape, chart)` puts a shape in a coordinate system: on the sea,
  in a whirlpool, on a body, through a bend or twist. Placement acts on skeletons by default (the lesson of the
  scene-graph proof: mapping every skin vertex through a growing twist shears bodies into slivers).
- **Multiply (a product of domains).** `sweep(curve, section)` is a surface (and a chart); `sheet(curveA, curveB)`,
  `loft(curves)`, `lathe(profile)`, `extrude`, `thicken(surface, height)` likewise; `points x shape` is instancing;
  `shape x time-interval` is a trail or a smear.

A strand is therefore not a primitive. It is `sweep(spine, section)` with frames transported along the spine.

### 7.3 A table is a function too

`tabulate(f, n)` samples a function and returns a function that interpolates the samples. The two are
interchangeable, and where the table is filled decides the schedule:

| Filled | Is today called |
|---|---|
| once, when the scene is built | a keyed curve, a baked mesh, a texture, a sculpt layer, a distance-field brick |
| once per frame on the CPU | the stations of a body (frames cannot be computed locally: they are transported along the spine) |
| never (the function is evaluated where it is used) | everything analytic |

So "bake or analytic" is one line of code per function, and a hand-edited table (a sculpt layer, a painted map) is the
same object as a computed one. In the expression layer this needs one new leaf, `fetch(table, index, lane)`, for
per-frame tables; interpolation is ordinary expressions on top. With it a body's chart is an expression like any
other: no hand-written chart shader, and differentiation, folding and CPU evaluation all pass through it.

### 7.4 Drawing

One generic drawable per signature, each assembled from expressions:

```ts
draw.surface(name, surface, { paint, grid, sides, outline })  // grid in the vertex stage; normals from the map itself
draw.curve(name, curve, { width, paint, segments })          // stroke
draw.points(name, points, thing)                             // instances of any instanceable drawable
draw.field(name, field, { bounds, paint, steps, lipschitz })  // sphere-traced (specified, not built)
draw.relief(name, surface, { height, hMax, fold, paint, steps }) // shell-traced on a surface (specified)
draw.density(name, density, { bounds, colour, steps })       // integrated (specified)
```

`strand`, `crest`, `fan`, `shell`, `sea`, `scatter`, `attach`, `decal`, `cloud`, `glow` become short library functions
that build maps and hand them to these. A baked mesh is `tabulate` of a `Surface` on a grid, so baking any surface is
generic.

### 7.5 What changed in the code (as built)

| Layer | Now |
|---|---|
| `src/expr.ts` | `fetch(table, index, lane)` (a leaf over a `TableSource`), `iterate(count, init, step)` (a real loop in both backends; `count` may be an expression); `leaves` reports tables; `substitute` rebuilds loops |
| `src/map/` | the signatures (`types.ts`), `warp` and `tile`, `place`/`on`/`compose`/`points`, the products (`sweep`, `sheet`, `lathe`, `loft`, `extrude`, `thicken`, `surface`), `tabulate`, frame transport |
| `src/draw/` | `draw.surface`, `draw.glazed`, `draw.curve`, `draw.points` (implemented); `draw.field`, `draw.relief`, `draw.density` (specified: options types, throw `NotImplementedError`); the sky and shadows |
| `src/geom/` | `strand`, `crest`, `sea` as library functions; `scatter`, `attach`, the chart WGSL and its CPU twin, the station reader in WGSL and its JS mirror, and every per-kind shader template are gone |
| `src/compile/` | slots, the table registry and packing, shared WGSL, the pose function; no kind of thing appears in it |
| `src/bake/` | bakes every `Drawable.surfaces` entry by sampling its expressions; colours converted to linear |
| contract, host, CLI, viewer | unchanged |

### 7.6 Where the first version of this section was wrong or awkward

- **One `warp` cannot dispatch.** Paint and fields are plain functions, so `warp(shape, F)` cannot tell them apart at
  run time. It is a namespace, one function per signature: `warp.paint`, `warp.field`, `warp.surface`, `warp.curve`,
  `warp.points`. A motif that needs the cell's identity uses `tile(F, motif)`, which hands it the whole fold result.
  The fold values are `src/lib/fold.ts`'s, whose result also carries `toWorld(d)` (inversion needs more than a stretch),
  so `warp.field` goes through that, not through `stretch`.
- **A slice is not a substitution.** Substituting `time` cannot move a per-frame table to another time, so a chart has
  `at(t)`, and a body taken at another time (`body.at(t)`) posts its own table, placed on its host's slice at t.
- **Placement acts on the local spine, and stretch is measured there.** The rest pose is the spine at time 0; a placed
  spine reads its host's table, which holds only the current frame, so stretch (and volume preservation) is measured on
  the unplaced spine. Exact for rigid placement (`on`); a bending chart would need its own rest table.
- **`paint` is three looks.** A lit surface takes `paint(u, v)` and the style; the sea needs its own colour and alpha,
  scene depth and the camera (`draw.glazed`, whose colour sees a `Fragment`: world point, pixel, (u, v), style, sun); a
  stroke is unlit (`draw.curve`). And "outline" is two things: an inverted hull for closed one-sided surfaces, ink along
  the v = 1 edge for two-sided sheets (which have no back faces to draw a hull with).
- **A surface needs more than a point.** It carries its `normal` (finite differences of the map by default; a sweep's is
  analytic, which is better near closing tips than any difference), its `topology` (periodic edges, poles, caps: what
  the baker needs to close a solid) and its `skeleton` (an empty chain for none).
- **A table needs a compile-time hook.** Fills evaluate CPU expressions (a body's spine, a host's chart), and only the
  compiler knows the slots, so a table is prepared once (`prepare(p)`, with `p.compile(roots)`). Each table compiles its
  own CPU function; the pose pass lost the sharing of subexpressions across bodies it had as one program (best-case pose
  0.31 -> 0.42 ms for the serpent).
- **Library functions that read scene depth cannot be shared.** `sceneWgsl` goes into every module, and scene depth is
  bound only for some draws, so each shader module carries its own library functions unless `sceneWgsl` already has them.
- **A sheet is ruled.** `sheet(root, edge)` interpolates linearly from root to edge, so the crest's sideways flutter is
  now a straight lean of the membrane, where it used to curve as f^2. Not visible in the pictures.
- **"Baking any surface" means any surface the CPU can evaluate.** The sea follows the camera and a stroke faces it;
  they bake to nothing and are reported as skipped. Static `tabulate` is evaluated at compile time with every input and
  param at its default.
- **Drawables are named.** A drawn shape needs a name (it keys its shader and its tables), so every `draw.*` takes one
  first; a sweep's name also names its table.
- **Params are global by name.** Two scenes declaring one param with different defaults cannot be loaded in one process
  (the shell demo and the serpent did, through `toon.outline`). Unchanged here; worth a scene-scoped registry.

### 7.7 Tracers: `draw.field` and `impostors` (as built)

Two per-pixel tracers (docs/folding_and_tracing.md 3.1 and 3.2), each one module in `src/draw/` sharing `trace.ts`.
Examples: `examples/coral.ts`, `examples/nest.ts` (fields), `examples/pearls.ts` (impostors).

```ts
draw.field(name, field, { bounds, paint, steps, outline, safety? })   // field: (p) => distance
impostors(name, { cell, shape, bounds, paint, cells, outline })      // shape: (cell) => sphere | capsule | cone
bound.box(min, max) | bound.sphere(c, r) | bound.capsule(a, b, r)    // TraceBound, a field's bound
fieldPixel                                                           // world size of a pixel: a field's level of detail
grow({ count, motif, step }), kifs({ count, fold, scale, offset })   // src/lib/grow.ts: folds as real loops
```

- **Proxy and ray.** The bound's oriented box is drawn as 36 vertices with front faces culled (a back face is visible
  whenever any of the bound is, the camera inside it included). The fragment stage builds the pixel's ray (one formula
  for perspective and orthographic: the proxy point moved back along `view_dir` to the plane through the eye) and its
  span in the bound's own shape, in closed form per kind.
- **Field march.** Steps by the distance (times `safety`, default 1); a hit is a distance under 0.3 pixels at that
  range, the pixel size being affine along the ray. Out of steps inside the bound counts as a hit. Two secant steps
  refine a hit (2e-4 of the analytic intersection in the tests; plain steps left 7e-3 at oblique hits). Normal: the
  symbolic gradient of the field (`grad.ts`), once per pixel. `paint({ p, normal })`; per-copy identity comes from
  evaluating the fold again at `p` (pure functions, so this is the same expression).
- **Level of detail.** `fieldPixel` is the pixel size at the point evaluated, per step, so an `iterate` count built
  from it falls with distance (coral: stop when a branch is under half a pixel). Per step, not per ray: per ray it
  was the pixel size at the bound entry, which is zero with the camera inside the bound.
- **Impostor walk.** A 3D DDA over the lattice cells (cells centred on multiples of `cell`, those with centres in
  `bounds`), each cell's shape intersected in closed form (Quilez's sphere, capsule and rounded-cone intersectors);
  the walk stops once the best hit is nearer than the current cell's exit. A cell's shape is an expression of
  `{ id, centre, hash }` and must stay inside its cell; radius 0 is an absent copy. `paint({ p, normal, local, cell })`.
- **In the scene.** Both write `frag_depth` (clip z / w of the hit), so they cut triangle bodies exactly and the sea's
  contact foam appears where they meet the water with no code of their own (seen in coral, nest and pearls). Lit by
  the style's `shade` with the underwater depth like any body. Opaque phase, no shadows cast (`skeletons` empty), not
  bakeable.
- **Edges.** A traced edge is inside its proxy, so MSAA does not see it. Coverage is written as
  `@builtin(sample_mask)` (round(4c) of 4 samples, bits turned per pixel), from how close a missed ray came in pixels,
  estimated between two samples as where their spheres of free space meet (the sampled distances alone gave grain).
  `outline: "silhouette"`: a ray passing within the ink width while receding is ink, including inner silhouettes.
  Ink width is the style's, at least 1.5 and at most 4 px (uncapped, a branch at the lens wore a 50 px band).
- **Safety check.** When the scene compiles, an unchecked field (`safety` absent) is sampled with `lipschitz` (20000
  pairs, in its bound plus 2%, at full and coarsest detail); above 1.01 it throws `FieldSafetyError` naming the field,
  the measured value and a suggested `safety`. A GPU-only function in the field is a `FieldSafetyError` too (cannot be
  checked; pass `safety`). It caught two real defects while building the examples: a polar repeat of coral trees whose
  crowns cross sector walls (98), and nested shells seen through windows with only one neighbour shell evaluated (12.9).

Corrections to the specification:

- **Derivatives after a march.** The style's shading calls `fwidth` after a loop whose exit differs per pixel, which
  WGSL's uniformity analysis rejects; the fragment entry points carry `@diagnostic(off, derivative_uniformity)`. Every
  invocation still runs there (a miss is discarded at the end, which demotes it to a helper), so the values are defined.
- **A fold that must see its neighbours costs them.** Repeats and polar repeats are exact only when the motif stays in
  its cell. A reef of three trees is three tree evaluations, not one polar fold, and the nest evaluates five shells per
  query. The check is what tells you.
- **Params are global by name** (7.6) bites examples: all three tracer examples use the default `toon.outline` so they
  load in one test process.

Cost, best case at 1280x800 on this (heavily loaded) machine, tracer only (frame minus the same frame without the
traced draw): coral about 11 to 14 ms at its overview cameras, 36 ms in the close-up that fills the screen, about 4 ms
at twice the camera distance and 23 ms at 0.6 times; nest about 1 to 7 ms (noisy); pearls about 3 to 6 ms. The step
cap (32 to 192 for coral, 16 to 96 cells for pearls) changes almost nothing: rays end by hitting, by the pixel
threshold or by leaving the bound well before the cap. Cost follows screen coverage times the field's cost per
evaluation (coral is 3 trees x up to 10 levels per step).
