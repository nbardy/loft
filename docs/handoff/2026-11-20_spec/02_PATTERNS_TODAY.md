# Patterns in old Loft: what an author could do, and how it composed

Each pattern: the API as it was, how it reached the GPU, how it met lighting, and what we would keep or change. File
paths are in the tagged tree `legacy/2026-11-19`.

## 1. Geometry: five ways to draw a thing, one depth

| Way | API | How it draws | Notes |
|---|---|---|---|
| Raster analytic surfaces | `strand`, `sweep(spine, section)`, `loft(curves)`, `lathe`, `extrude`, `thicken`, `draw.surface(name, surface, { paint, grid })` | a (u, v) grid posed in the vertex stage, painted per pixel, ink outline hull | swept bodies have analytic normals; frames along a spine are a scan (parallel transport), computed per frame by pose stages |
| Sphere-traced fields | `draw.field(name, field, opts)` | the field's bounding box rasterised; each pixel sphere-traces inside it; refinement to ~2e-4 of the analytic hit | needs a Lipschitz bound ("safety") when the field reads textures; many-part fields were N-way unions (slow) |
| Relief | `draw.relief` | shell-traced height over a surface | |
| Analytic impostors | `impostors`, `draw.copies` with sphere/capsule/cone, `draw.traced` for meshes and solids | one exact ray-primitive solve per lattice cell or listed copy | the coral test: listed analytic branches 8× faster than a bricked march |
| Meshes | `draw.mesh` | triangles | none come in; bake writes glTF out |
| Instancing | `draw.posed`, `draw.copies` (minPixels, keepLevels), `draw.points` ("specks") | instanced draws, LOD by screen size | |
| Media | `draw.density` (analytic Gaussian), `draw.volume` (marched extinction field, HG phase `g`, shadow march) | per-pixel integration up to opaque depth | each volume one composited layer in author order |

All of them write the same depth, so a field and a surface occlude each other; fields write projected hit depth.

## 2. Space folding and domain maps

`warp(shape, F)`: a `Warp` returns `{ x, id, stretch }` (mapped coordinate, the cell it came from, the local change of
length so distances stay distances). On a field or paint it is folding: mirror, repeat, polar repeat, spiral,
kaleidoscope. On a surface or curve it reparametrises. On points it maps the index (a generator of instances).
`place(shape, chart)` puts a shape in a coordinate system (on the sea, in a whirlpool, on a body); placement acts on
skeletons by default. Products: `sweep`, `sheet`, `loft`, `lathe`, `extrude`, `thicken`; "points × shape" is
instancing; "shape × time" a trail. A table is a function too: `tabulate(f, n, when)` fills at build, per frame or
never. Detail in `FOLDING AND TRACING` below.

What the draft fixes: domain maps did not create spaces, so a point in a mirrored, offset space could be used as a body
point (the gaze bug); cells were ids without a typed map back.

## 3. How fields and surfaces met PBR

There was no separate material system. A thing's `paint` (a function of its coordinates, or a struct of "words":
`base`, `gloss`, `metal`, `emits`, `rough`, `normal`) reached the style's `shade(ctx)` with a `ShadeCtx` (paint,
normal, sun, view, world point, the light fold `lights(combine, term)`, `channel(word)`, `scatter(toward)`, `sky(dir)`).
A field hit and a raster fragment produced the same `ShadeCtx`, so a traced claw and a swept leg shaded the same way.

`pbr(parts)` is a style built from swappable parts:

```ts
type PbrParts = {
  distribution: (n_h, rough) => F;                    // ggx
  visibility:   (n_l, n_v, rough) => F;               // smithGgx
  fresnel:      (v_h, f0) => P3;                       // schlick
  diffuse:      (albedo, n_l, n_v, v_h, rough) => P3;  // lambert
  environment:  (at, rough, sky) => { diffuse, specular };   // skyLight (split sum)
  occlusion:    (ctx) => F;                            // distance-field AO, or 1
  sky:          (dir, sun) => P3;                      // daylight
  tone:         (c) => P3;                             // aces
  lobeAlbedo:   (n_v, rough) => { scale, bias };       // integrated from distribution+visibility on the twin
  clear:        [r, g, b];
};
```

Lights: the sun plus up to seven `lights`, each a function of a world point returning `{ toward, amount, distance?,
size? }`; a lamp may cast depth-map shadows. A style never sees a list: it folds them.

Known flaws (from the critiques): single-scattering GGX loses energy at high roughness; the sky reflection cannot show
off-screen objects; AO differs between fields and raster; shadows on water came from capsule skeletons; volumes
composite in author order; light transport is welded to `shade`.

## 4. Breaking out of PBR (stylised looks)

A `Style` is one value: `shade`, `volume`, `ink`, `mark` (alpha for post), `water`, `shadow`, `sky`, `post` (full-screen
WGSL passes in order), `block`, `lights`. Shipped styles: `toon`, `woodblock` (with named lightings: flat, twoTone,
threeTone, bokashi, hatched, contact, rim, cel), `biolume`, `papercraft`, `sketch`, `neon`, `pbr`. Each reads only the
paint words it wants and notes the ones it ignores. `sketch` is written out in full in `docs/styles.md` as the
template for a new style. Post passes read the picture and its depth (biolume's reflection, papercraft creases).

Ways out, from smallest to largest:
1. Replace one PBR part (`pbr({ distribution: myLobe })`).
2. Write a new `Style` (any `shade`, `ink`, `post`).
3. Paint returns its own words; a style of your own reads them.
4. A raw WGSL function anywhere in the graph (`dpdx`, textures, integers): emitted as written, opaque to the twin.
5. `draw.shader(name, wgsl, { vertices, instances, phase, blend, depthWrite, cull })`: your own shader with the scene's
   camera, depth and tables.
6. `compute(name, { when: build|load|frame|steps, writes, dispatch, wgsl })`: any GPU program, outputs readable by
   everything else.
7. `--engine realtime|reference|print|inspect`: the design slice's pass lists (ordered scenes).

What was missing: forking one shader of one kind while keeping the rest; replacing the visibility record or the light
transport without rewriting a style; a stylised look that still gets correct shadows and reflections for free.

## 5. Escape hatches and seeing the code

- `wgsl` text with `${}` holes lowers a WGSL subset into the graph (loops, if, struct returns, swizzles); outside the
  subset a function is raw and named opaque; errors quote the author's line; `loft show` prints every module with
  `/* file:line */` comments.
- Holes bring params, inputs, solved curves, tables and other functions in; strings are refused.

## 6. Motion: what existed

| Piece | API | What it is |
|---|---|---|
| Curves | `keys(points, { interp })`, `bspline(t, over, values)` | keyed and B-spline curves of time |
| Gait | `gait(spec)` | a walk clock: feet lift and plant on a phase (duty ~0.66); bob, sway and roll are sines of that clock |
| Stance | `src/geom/stance.ts` | a sum type of carriages (idle, brace, dance, walk) blended by envelopes; feet planted at homes, moved only by named events (re-plants, stamps, walk) |
| IK and frames | `limb`, `chain(spec)`, `poseFrame`, `turnedAbout` | analytic three-segment limb IK; knees fold when the body moves over planted feet |
| Solve | `solve({ over, vary: { x: spline(n, from) }, hold: (u) => [always([time, a, b], equal(...)), ge(...)], prefer: { jerk \| acceleration \| near } })` | cubic B-spline unknowns; holds on position, velocity, acceleration; least jerk; augmented Lagrangian with Newton steps and exact Hessians; runs once at build; holds `Unproved` in builds, proved by `loft check` |
| Recurrences | `recur`, `iterate`, compute `steps` | stateful simulation with kept states and checkpoints |
| Weight-driven stepping (look round) | a `recur` over 8 feet at 120 steps/s | capture-point stepping from where the foot is; lean ≈ atan(a/g) |
| Gaze and neck (look round) | head placed per frame (level gaze, glance ≤ 15°), neck a fixed-length curve between shoulder and head frames | |
| New motion slice (`src/motion`) | `body({ trunk, chains: [{ root, lengths, end: foot(at) }] })`, `move(body, roam(laws?), { go: input(...), script })`, `roam({ arch?, step?, present?, deck? })` | laws are WGSL files with value-only interfaces (`seams`, `support`, `share_range`, `pin`, `deck`, `roam`, `present`); replace one law or the whole module; all creatures in one fused dispatch; live equals bake to the bit |

The new motion slice's whole crab:

```ts
import { body, foot, move, roam } from "loft/motion";
const legs = [[1.35,2.8,6.6],[0.5,1.1,7.4],[-0.4,-0.9,7.3],[-1.3,-2.7,6.4]] as const;
export const crab = body({
  trunk: { size:[4.8,1.05,5], height:2.5 },
  chains: [-1,1].flatMap(s=>legs.map(([x,fx,fz])=>({ root:[x,0.1,2.5*s], lengths:[3,2.9,1.2], end:foot([fx,0,fz*s]) }))),
});
export default stage(move(crab, roam(), { go: input("stick","vec2",[0,0]), script: stick }));
```

## 7. Mixing bespoke and physical motion (what worked)

The heron that read best: the **body** solved near the animator's keys (keys are the prior, solve adds the flight arc
and contacts), the **head** placed per frame by a gaze law in world space, the **neck** solved per frame between them.
The crab that read best: the **trunk** follows a directed path, the **feet** step from weight, the swings vary with
urgency. In both, authored intent sets the targets and physical laws fill in weight and contact. The open question for
the final design is how to make that layering a first-class, rewritable composition (see `MOTION HANDOFF`).
