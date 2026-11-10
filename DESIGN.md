# Loft: API design (v0)

Loft is a way of describing animated 3D things as code, and a compiler that turns the description into shader source
plus tables of numbers that any WebGPU host can draw. The reasoning behind it is in
`room-runners-arena-lib`, `notes/design/2026-11-10_creature_pipeline_options.md`, sections 10 to 12. This file is the
contract for building it.

## 1. What is being built

```
 authoring code (TypeScript)          the compiler (TypeScript)             a host
 builds a scene description   --->    scene -> Artefact              --->   draws an Artefact
 src/geom, src/scene, src/style       src/compile                           src/host (WebGPU: browser, bun headless)
                                      Artefact = WGSL + tables + draw list  later: Rust/wgpu inside the game
```

Nobody compiles TypeScript to anything. TypeScript runs normally and *builds a graph of expressions*; the compiler
turns that graph into WGSL (for the GPU) and JavaScript (for the once-per-frame CPU pose pass). The game would load
an Artefact; it never runs authoring code.

Five ideas, in the order the code depends on them:

1. **Expressions** (`src/expr.ts`, done). Authoring code builds `Expr` values. One DAG, two backends (JS, WGSL), tested
   to agree.
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

Open types: a "shape" is any object that implements one or more capability interfaces (section 4). Functions ask for
capabilities, never for concrete classes. The one rule: everything is pure.

## 2. Authoring API (target)

```ts
import { compile, crest, keys, palette, param, scene, sea, strand, through, time, toon, vec3, wave } from "loft";

const girth = keys([[0, 0.3], [0.06, 0.9], [0.45, 1.0], [0.85, 0.4], [1, 0.02]]);   // a table-backed curve

const body = strand({
  name: "serpent",
  // The spine: any function of m (and time). `through` is a spline through control points, which may move.
  spine: (m) => through(m, [[-9, 0.4, 0], [-4, 0.9, 1.5], [0, 0.6, -1.2], [4, 1.8, 0.4], [7, 3.2, 0]])
                  .add(wave(m, { axis: [0, 0, 1], waves: 1.6, rate: 0.5, grow: keys([[0, 0.1], [1, 1]]) })),
  stations: 40,
  girth: (m) => girth(m).x.mul(param("serpent.girth", 1.1)),
  section: { aspect: (m) => keys([[0, 1.15], [1, 0.7]])(m).x, squareness: 2.3 },
  paint: (m, v) => palette.countershade(v, { back: [0.16, 0.13, 0.33], belly: [0.86, 0.82, 0.62] }),
});

const fin = crest({ on: body, from: 0.12, to: 0.8, height: keys([[0, 0], [0.3, 1.2], [1, 0]]), paint: [0.75, 0.2, 0.28] });

export default scene({
  sea: sea({ swell: [{ direction: [1, 0.3], length: 14, height: 0.25, speed: 1.1 }] }),
  things: [body, fin],
  style: toon({ bands: 3, outline: 0.045 }),
  sun: [-0.6, 0.5, -0.6],
  cameras: { three: { kind: "perspective", eye: [-14, 9, -13], target: [0, 1, 0], fovY: 0.6 } },
});
```

`compile(scene) -> Artefact`. A scene module's default export is a `Scene`.

### 2.1 Curves (`src/curve.ts`)

- `type Curve<T> = (x: In<"f32">) => Expr<T>`.
- `keys(points, { interp?: "smooth" | "linear" })`: keyed scalar or vector curve, sampled into a table row
  (`CURVE_N` samples, vec4 each). Returns `Curve<"vec4">`; use `.x`, `.xyz`.
- `ramp(colours)`: evenly spaced colour stops, table-backed.
- `through(m, points)`: an interpolating cubic spline through control points that may themselves be expressions
  (so a spine can move). Returns `Expr<"vec3">`. Closed form (a sum of basis weights), no table.
- Motion operators, all closed-form in time (no state): `wave(m, {...})` (a travelling lateral wave with an amplitude
  envelope), `retarded(t, m, speed)` (= `t - m / speed`: follow-through with no state), `response(t, { f, zeta, r })`
  (step response of a second-order system: easing with anticipation when `r < 0` and overshoot when `zeta < 1`),
  `pulse(t, t0, t1)`.

### 2.2 Bodies (`src/geom/`)

- `strand(spec) -> Strand`. Spec: `name`, `spine(m)`, `stations` (how many frames the pose pass computes), `girth(m)`
  (half-width), `section` (`aspect(m)` = height over width, `squareness` superellipse exponent), `twist(m)`,
  `tips` (`"round" | "point" | "open"` at each end), `paint(m, v)`, `detail` (density hints).
  A `Strand` is a `Surface`, a `Chart`, and has a `Skeleton`.
  - Frames along the spine are rotation-minimising (double reflection, Wang et al. 2008), computed in the pose pass.
  - Volume is preserved: where the spine is locally shorter than at rest by a factor k, radii scale by 1 / sqrt(k).
  - Vertices are generated in the vertex shader from the station table and the curve tables. Grid density is a WGSL
    `override`, so level of detail never touches the description.
- `crest({ on, from, to, height, v?, paint, sag?, flutter? }) -> Sheet`: a membrane standing on a strand's surface
  along a line of constant `v`.
- `fan({ rays, sag, scallop, paint }) -> Sheet`: a membrane between rays (a wing, a fluke). Rays are spines.
- `shell({ profile, turns?, growth?, angular?, paint }) -> Shell`: a lathe; with `turns` and `growth` a spiral shell
  or horn (Raup's parameters).
- `scatter({ name, on, count, at(i), thing, scale? }) -> Scatter`: instances of `thing` placed at body coordinates
  `(m, v, rho)` on a host and oriented by its chart's frame (done; the example's teeth and dorsal spikes).
- `mesh({ positions, indices, paint }) -> Mesh`: explicit triangles are a full citizen.

### 2.3 Charts and the scene graph (`src/chart.ts`)

```ts
interface Chart {
  /** Local coordinates to world. Always available. */
  toWorld(local: Expr<"vec3">): Expr<"vec3">;
  /** World to local, where the chart has an inverse. */
  toLocal?(world: Expr<"vec3">): Expr<"vec3">;
}
compose(outer, inner): Chart
place(thing, { in: chart, how: "deformed" | "rigid", at?: [..] })
attach(thing, { to: strand, m, v, rho })    // done: the example's head, jaw, horns and eyes
```

- `sea(...)` is the root chart: `(x, z, height above the surface)`, with a swell that moves in time. It is also a
  `Surface` (the water) and a `Canvas` for pixel-time decals.
- `whirlpool({ at, radius, spin(r), depth(r), drift })` is a chart: a twist that accumulates with time, a funnel and
  an inward drift, the identity beyond its radius.
- `deformed`: every vertex of the child is mapped through the chart in the vertex shader (it bends with the parent).
  `rigid`: the child keeps its shape; its position and orientation come from the chart's value and Jacobian.

### 2.4 Space-time (`src/spacetime.ts`)

```ts
interface SpaceTime<B> { readonly span: readonly [number, number]; at(t: In<"f32">): B }
telegraph(obj, { arm, style })     // the footprint on the sea of obj.at(arm): a toothed marker, pixel-time
smear(obj, { over, samples })      // ghosts of the recent past, fading with age
wake(obj, { over })                // foam where the body crossed the water in the recent past
```

A body whose spec reads `time` is already a space-time object; `at(t)` substitutes the time. The game simulation owns
hit shapes and timings: an attack is built *from* the sim's capsule and arming time, and the telegraph is derived
from the body, so they cannot disagree.

### 2.5 Soft things, evaluated per pixel (`src/field.ts`)

- `decal(canvas, colour(local))`: a function painted on a canvas (the sea), antialiased by screen-space derivatives.
  Cast shadows (capsule shadows from the skeleton), wakes and telegraphs are decals; each is drawn as its own small
  patch of the sea surface in the `"on-water"` phase, so its cost is the area it covers. Foam where the water meets a
  body comes from scene depth (section 3).
- `cloud(blobs)`: Gaussian blobs integrated analytically along the view ray. No marching, no sorting.
- `glow(polyline, { width, colour })`: falloff of distance to a line (lightning, beams).

### 2.6 Style (`src/style.ts`)

A style is a program in the same language: `{ shade(ctx) -> colour, ink, outline, water(ctx), shadow(ctx), sky(ctx), post }`
where `ctx` carries base colour, normal, sun and view directions, body coordinates `(m, v, rho)`, world position and
depth below the surface. Styles are swappable without touching a scene. v0: `toon`. Planned: `woodblock`,
`papercraft`, `ink`, `biolume`.

### 2.7 Checks (`src/check.ts`)

Assertions evaluated on the CPU from the same description: `clearance(a, b, { min, over })` (capsule distances
through time), `minScreenSize(feature, camera, px)`, `budget({ triangles })`.

### 2.8 Local form, sculpt layers and the round trip with meshes (M2)

Form is layered, coarse to fine, and every layer above the first is expressed in the body's own coordinates
`(m, v)`, so it rides the body through animation, re-posing and changes of density, and editing a lower layer does
not destroy a higher one (the multiresolution idea behind sculpt layers in ZBrush, Mudbox and Blender's Multires).

| Layer | Form | Edited by |
|---|---|---|
| gesture, mass, section | curves (`spine`, `girth`, `section`) | code |
| local form | `stamp`s: analytic bumps, creases and ridges placed at `(m, v)` with a radius and a falloff (the analytic counterpart of sculpt brushes: draw, inflate, crease, pinch, flatten) | code |
| free form | a sculpt layer: a displacement table over `(m, v)` (scalar along `rho`, or a vector in the body frame for overhangs) | a person in a sculpting tool, or a fit from an imported mesh |
| paint | functions and tables over `(m, v)` | code, or painted |

Round trip:

- **Out:** `bake(body, density) -> glTF` with the body coordinates as the UV map (the unwrap is free) and skin weights
  from the stations. Any sculpting tool can open it.
- **Back:** the difference between the sculpted mesh and the base, baked into that UV map, is the sculpt layer.
  Standard displacement baking.
- **Absorb:** `fit(mesh, skeleton)` casts a ray outward from the spine for each `(m, v)` and records where it meets
  the mesh, giving a radius function. Its smooth part is re-fitted into `girth` and `section` curves and the residual
  becomes a sculpt layer. This is how a generated or hand-made mesh becomes a Loft body. It is valid only where each
  ray meets the surface once (the same limit as folding).

Operators borrowed from modelling tools, as functions in the same language: `mirror`, `array` along a curve,
`solidify` (thickness for a sheet), `bend` / `twist` / `taper` and `lattice` (deformers are charts), `shrinkwrap`
(attach at a host's `rho`), `displace`, `cut` (remove where another body's `rho < 1`). Shader vocabulary still to add
to the expression layer: gradient and cellular noise with fractal sums, `remap`, polar coordinates, a Fresnel term,
access to scene depth (for foam where things meet and soft volume edges), and a raw-WGSL node as an escape hatch.

## 3. The Artefact (`src/artefact.ts`, done)

Read that file; it is the host contract. In short: a fixed prelude (bind group 0: frame, inputs, params, curves), an
artefact-declared bind group 1 of per-frame tables, complete shader modules, a list of buffer-less draws, optional
post passes, and `pose(state)` which computes the per-frame tables on the CPU. `FrameState.curves` is the curve table
the host holds now, so a `setCurves` edit moves the CPU pose exactly as it moves the GPU.

Per-frame tables are declared by drawables (`ctx.table`, section 4.1); the compiler packs them, in declaration order,
into as few storage buffers as hold them (`loft_tables_0`, ...), because a shader stage may bind 8 storage buffers
and group 0 uses 3. Each table is read through its accessor, `t_<name>(i: u32) -> vec4<f32>`, which adds its offset.
More than 5 buffers' worth (or one table over 128 MiB) fails at compile time with `TableBudgetError`. The tables the
built-in kinds declare (all `array<vec4<f32>>`):

| Table | Declared by | Per entry | Contents |
|---|---|---|---|
| `stations` | the compiler, for every slice any drawable asked for | 3 vec4 | `p` (xyz position, w arc fraction), `q` (frame quaternion), `r` (half-width, half-height, stretch, unused) |
| `shadows` | `src/draw/shadows.ts`, from every `Skeleton` | header + 3 vec4 | capsules projected along the sun onto the sea: ends, radii, strength, softness, chain links |

A slice is a strand at a time (`time`, `time + offset`, an input): each is its own range in `stations`.

Foam where the water meets a body is not a table: the sea reads the opaque scene's depth (`Draw.sceneDepth`) and
foams where an opaque surface lies within the foam width in 3D (a ring of taps at that width on screen). It replaced
the CPU waterline "collar" table: it needs no per-kind code (any opaque thing gets foam), its cost per sea pixel is
fixed, and it follows the drawn surface exactly. What it cannot see is a waterline hidden behind the body, which the
camera cannot see either.

## 4. Capabilities (the open type system)

```ts
interface Surface   { surface(st, m, v): SurfacePoint }                 // pos, normal, radial: can be sampled into triangles
interface Chart     { toWorld(...); toLocal?(...) }                     // is a coordinate system
interface Skeleton  { skeleton(ctx: CompileContext): StationRange }     // capsules: shadows, collision, footprints
interface Attachable<T> { placed(p: Placement): T }                     // can ride a host's chart (`attach`)
interface Field     { at(p: Expr<"vec3">): Expr<"f32"> }                // has a value at a point (planned)
interface Drawable  { readonly name: string; draws(ctx: CompileContext): Emitted }   // knows how to schedule itself
```

A new kind of thing is a new object implementing some of these. `Drawable` is the extension point for the compiler:
a type that can emit its own shaders and tables needs no change to the compiler. `Strand`, `Crest`, `Scatter`, `Sea`,
the sky and the shadows are all plain drawables (`src/geom/`, `src/draw/`); `scene()` adds the sky, the sea and the
shadows in front of the scene's things.

### 4.1 What a drawable can ask for (`src/compile/context.ts`)

```ts
interface CompileContext {
  readonly slots: Slots;            // emit expressions as WGSL: emitFn(ctx.slots, name, args, ret, roots, combine)
  readonly style: Style; readonly sea: Sea; readonly sun: V3;
  readonly things: readonly Drawable[];                        // for derived drawables (shadows of every Skeleton)
  table(spec: { name; vec4s; fill(frame: PoseFrame, out: Float32Array) }): TableRef;   // TableRef.wgsl = accessor
  evaluate(roots: readonly Expr[]): CpuRef;                    // per-frame CPU values; read with frame.values(ref)
  stations(strand: Strand, at?: Expr<"f32">): StationRange;    // a slice: default `time`
  chart(strand: Strand, at?: Expr<"f32">): ChartRef;           // WGSL fn (m, v, rho) -> BodyPoint, and its CPU twin
  shared(key: string, wgsl: string): void;                     // module-scope WGSL in sceneWgsl, once per key
  post(pass: { label; wgsl }): void;                           // a full-screen pass (the style's come first)
}
interface PoseFrame { env; table(ref): Float32Array; values(ref); seaHeight(x, z) }    // what a fill sees
type Emitted = { shaders: Record<string, string>; draws: DrawSpec[] };                   // DrawSpec = Draw with a phase
type Phase = "background" | "opaque" | "water" | "on-water" | "transparent";
```

- **Order** is stated, not wired: a draw names its phase; opaque phases run first, then transparent; within a phase,
  draws run in the order drawables emit them. Anything laid on the water after it is `"on-water"`.
- **Scene depth**: a transparent draw sets `sceneDepth: true` and may call `scene_depth(px)` / `scene_position(px)`.
- **Charts.** `BodyPoint` is the point at `(m, v, rho)`, the skin normal at `(m, v)`, and a frame: `x` the spine
  tangent, `y` the outward direction at `v` within the section (no taper lean), `z = x cross y`. `attach(part, { to,
  m, v, rho })` authors the part in that frame and the pose pass places it on the host's chart every frame (exact,
  at every slice); `scatter({ on, count, at(i), thing, scale })` places copy `i` (`instance_index`) on it in the
  vertex stage. `compileScene(scene)` returns the artefact plus `chart(strand, at)` and `frame(state)`, so checks and
  tests can evaluate the CPU twins over a frame.
- **The CPU backend** (`compileJs`) is scalarised (no array per vector op) and folds constants as it emits, so the
  pose pass is plain arithmetic on what actually varies.

### 4.2 Writing a new kind of thing

One module. This one draws a ring of markers on the water around a strand's head, from a table the pose pass fills:

```ts
import { draw, type CompileContext, type Drawable, type Emitted, type Strand } from "loft";

export function headMarker(body: Strand): Drawable {
  return {
    name: "headMarker",
    draws(ctx: CompileContext): Emitted {
      const chart = ctx.chart(body);                                  // the body, posed every frame
      const t = ctx.table({ name: "headMarker", vec4s: 1, fill: (frame, out) => {
        const p = chart.cpu(frame)(0, 0.25, 0).pos;                    // the head, on the CPU
        out.set([p[0], p[2], 1.5, 0]);                                 // centre xz, radius
      } });
      const code = /* wgsl */ `
struct VOut { @builtin(position) clip: vec4<f32>, @location(0) world: vec3<f32> };
@vertex fn vs(@builtin(vertex_index) vi: u32) -> VOut {
  let m = ${t.wgsl}(0u);
  let c = vec2<f32>(quad_corner(vi)) * 2.0 - 1.0;
  let xz = m.xy + c * (m.z + 0.2);
  var out: VOut;
  out.world = vec3<f32>(xz.x, sea_height(xz), xz.y);
  out.clip = frame.view_proj * vec4<f32>(out.world, 1.0);
  return out;
}
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> {
  let m = ${t.wgsl}(0u);
  let d = abs(distance(in.world.xz, m.xy) - m.z);
  return vec4<f32>(1.0, 0.3, 0.2, 1.0 - smoothstep(0.05, 0.08, d));
}`;
      return {
        shaders: { headMarker: code },
        draws: [draw({ label: "head marker", shader: "headMarker", vertices: 6, phase: "on-water",
                       cull: "none", depthWrite: false, blend: "alpha" })],
      };
    },
  };
}
```

Add it to a scene's `things`. Nothing in `src/compile` changes: the compiler packs the table, binds it, fills it each
frame after the stations it reads, and runs the draw after the sea. A telegraph is the same with
`ctx.chart(body, input("attack.arm", "f32", [0]))`.

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

Sections 2.2 to 2.5 describe kinds of thing (strand, crest, sea, decal, cloud), each with its own hand-written shader
template. Extending Loft to folding, to arbitrary analytic surfaces, to points and to traced solids kind by kind would
multiply those templates. This section replaces the kinds with one small algebra. Sections 2.2 to 2.5 remain as the
library of *names* built on it.

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
draw.surface(surface, { paint, density, sides, outline })   // grid in the vertex stage; normals from the map itself
draw.curve(curve, { width, paint })                          // stroke
draw.points(points, thing)                                   // instances of any drawable
draw.field(field, { bounds, paint, steps })                  // sphere-traced
draw.relief(surface, { height, fold, steps })                // shell-traced on a surface
draw.density(density, { bounds })                            // integrated
```

`strand`, `crest`, `fan`, `shell`, `sea`, `scatter`, `attach`, `decal`, `cloud`, `glow` become short library functions
that build maps and hand them to these. A baked mesh is `tabulate` of a `Surface` on a grid, so baking any surface is
generic.

### 7.5 What changes in the code

| Layer | Change |
|---|---|
| `src/expr.ts` | `fetch` for per-frame tables; a loop form `iterate`; `src/grad.ts` adds symbolic differentiation |
| `src/map/` (new) | the signatures, `warp`, `place`, the products, `tabulate` |
| `src/draw/` | the generic drawables; the per-kind templates in `src/compile/shaders.ts` are deleted as their kinds move over |
| `src/geom/`, `src/chart.ts` | become library functions over `src/map/` |
| `src/bake/` | bakes any `Surface` by sampling it; body discovery by probing is deleted |
| contract, host, CLI, viewer | unchanged |

