# Geometry an author makes, and rays an author casts

A design proposal, written 2026-11-13. Nothing here is built. It obeys the nine rules of `docs/open_design.md`
and argues against none of them. Numbers marked *measured* come from the throwaway experiments in `out/mesh_probe/`
(not committed), run on this machine with the load average near 10. Frame times swing about 2x between the GPU's
clock states, so every number is "best of 30 or 40 frames". *Predicted* numbers are what a build will be scored
against.

## The short answer to the owner's four questions

- **Triangles and quads.** Loft draws triangles all the time. Every surface is a grid of triangles that the vertex
  stage computes from the vertex's index; there is no vertex buffer anywhere. Static fields can be meshed by surface
  nets, and the bake command writes any scene out as glTF. What is missing is **input**: an author's own algorithm
  (a hull, a shatter, a subdivision) has no supported way to hand Loft its triangles. Quads are not a primitive
  (WebGPU has none): a grid cell is two triangles.
- **Distance-field shapes.** Yes. `draw.field` sphere-traces a signed distance field per pixel inside a bound. It
  checks the field on the CPU first, and it has brick and mesh schedules. The primitives and folds are in
  `src/lib/sdf3.ts`, `fold.ts` and `similar.ts`.
- **Ray marching.** Yes, for **primary rays only**: sphere tracing (`draw.field`), a march down a thin shell in a
  body's own coordinates (`draw.relief`), and a cell walk (`impostors`). Media are integrated in closed form along
  the view ray, without marching (`draw.density`).
- **Ray tracing.** Yes for primary rays: exact closed-form hits of spheres, capsules and rounded cones (`draw.copies`,
  `impostors`). There is one secondary ray, and an author cannot reach it: the style's shadow block casts from each
  opaque pixel toward the sun against the skeletons' capsules. Reflections on water are a screen-space smear in a post
  pass. There are no reflection or refraction rays, and no way for an author to cast a ray against the scene.

The proposal adds four names and two types:

- `draw.mesh`: triangles from the author's code, moved by an expression.
- `draw.traced`: any intersect-or-march function as a drawable.
- `march`: a field's sphere trace, as a function.
- `castRay`: a ray against every thing's skeleton capsules.

Its central claim: **in Loft a tracer is a function, so every tracer is already a secondary ray.**

---

## 1. What an author writes

Three scenes, written before the design and read cold. The questions a stranger asked on reading them, and what
changed in the design because of each, follow the scenes.

### 1.1 A pane that shatters: triangles from the author's own algorithm

```ts
// A pane of glass breaks at t = 0.4 and falls as 36 shards. The shards are Voronoi cells of the pane, clipped in
// plain TypeScript at build, each made a thin prism. They fall and spin by an expression of time, so any frame
// draws alone.
import { add, distance, draw, f, max, min, mix, mul, normalize, plane, rotateAbout, scene, sin, cos, sqrt, sub, time, toon, vec2, vec3, type F, type P3 } from "../src/index.ts";

type Pt = [number, number];
const H = 0.015; // half the glass's thickness
const seeds: Pt[] = Array.from({ length: 36 }, (_, k) => [Math.sin(k * 12.99) * 0.95, 1 + Math.sin(k * 78.23) * 0.95]);

/** Seed i's cell: the pane's rectangle clipped to the half-plane nearer seed i than each other seed. */
function cell(i: number): Pt[] {
  let poly: Pt[] = [[-1, 0], [1, 0], [1, 2], [-1, 2]];
  const [ax, ay] = seeds[i]!;
  seeds.forEach(([bx, by], j) => {
    if (j === i) return;
    const nx = bx - ax, ny = by - ay, c = (nx * (ax + bx) + ny * (ay + by)) / 2;
    const side = (p: Pt) => nx * p[0] + ny * p[1] - c;
    poly = poly.flatMap((p, k): Pt[] => {
      const q = poly[(k + 1) % poly.length]!, sp = side(p), sq = side(q);
      const cut: Pt[] = sp * sq < 0 ? [[p[0] + (q[0] - p[0]) * sp / (sp - sq), p[1] + (q[1] - p[1]) * sp / (sp - sq)]] : [];
      return sp <= 0 ? [p, ...cut] : cut;
    });
  });
  return poly;
}

// One row per corner: rest position x y z, the shard's centre cx cy, its index k, and 1 on the rim (0 on the faces).
const points: number[][] = [];
const triangles: [number, number, number][] = [];
const corner = (row: number[]) => points.push(row) - 1;
seeds.forEach((_, k) => {
  const poly = cell(k);
  const cx = poly.reduce((s, p) => s + p[0], 0) / poly.length, cy = poly.reduce((s, p) => s + p[1], 0) / poly.length;
  for (const z of [H, -H]) {
    const fan = poly.map((p) => corner([p[0], p[1], z, cx, cy, k, 0]));
    for (let j = 1; j + 1 < fan.length; j++) triangles.push(z > 0 ? [fan[0]!, fan[j]!, fan[j + 1]!] : [fan[0]!, fan[j + 1]!, fan[j]!]);
  }
  poly.forEach((p, j) => { // the rim: its own corners, so it shades flat against the faces
    const q = poly[(j + 1) % poly.length]!;
    const a = corner([p[0], p[1], H, cx, cy, k, 1]), b = corner([q[0], q[1], H, cx, cy, k, 1]);
    const c = corner([q[0], q[1], -H, cx, cy, k, 1]), d = corner([p[0], p[1], -H, cx, cy, k, 1]);
    triangles.push([a, d, c], [a, c, b]);
  });
});

/** Where a corner is at `time`: each shard comes loose later the further it is from the blow at (0, 1.2), then spins
 *  about its centre and falls until its centre is 5 cm above the ground (it stops there; shards do not stack). */
const at = (r: readonly F[]): P3 => {
  const [x, y, z, cx, cy, k] = r as [F, F, F, F, F, F];
  const loose = add(0.4, mul(0.15, distance(vec2(cx, cy), vec2(0, 1.2))));
  const s = min(max(sub(time, loose), 0), sqrt(mul(sub(cy, 0.05), 1 / 4.9))); // seconds falling, up to landing
  const centre = vec3(cx, cy, 0);
  const turned = rotateAbout(sub(vec3(x, y, z), centre), normalize(vec3(sin(k), 0.4, cos(k))), mul(s, 3));
  return add(add(centre, turned), vec3(mul(cx, mul(s, 0.6)), mul(mul(s, s), -4.9), mul(s, 1.2)));
};

const pane = draw.mesh("pane", { points, triangles }, {
  at,
  paint: (row) => mix(vec3(0.55, 0.75, 0.8), vec3(0.92, 0.97, 1), row[6]!), // the rim reads lighter
  sides: "one",
  outline: "hull",
  // One short capsule per shard, carried by the same `at`, so each shard casts its own shadow.
  skeletons: seeds.map((_, k) => {
    const c = points.find((p) => p[5] === k)!;
    return { joints: 2, joint: (j: number) => ({ p: at([c[3]! + (j - 0.5) * 0.1, c[4]!, 0, c[3]!, c[4]!, k].map(f)), r: f(0.08) }) };
  }),
});

export default scene({
  ground: plane, things: [pane], style: toon(), sun: [-0.4, 0.8, 0.45], period: 3,
  cameras: {
    whole: { kind: "perspective", eye: [3.5, 2.2, 4.5], target: [0, 0.9, 0], fovY: 0.7 },
    blow: { kind: "perspective", eye: [0.8, 1.4, 1.6], target: [0, 1.2, 0], fovY: 0.5 },
  },
});
```

The algorithm (Voronoi clipping) is ordinary TypeScript and runs once, at build. The motion is an expression of the
row and `time`. The two never mix, which is what keeps "any frame draws alone".

### 1.2 A traced solid nobody put on a menu: a bored bead, by intervals

```ts
// A ball bored through along y: the sphere's span along the ray minus the bore's span. Exact, with no march.
import { add, and, bound, div, draw, dot, ge, length, lt, max, mul, neg, normalize, plane, scene, select, sqrt, sub, toon, vec2, vec3, type Ray, type Traced } from "../src/index.ts";

const C = vec3(0, 1, 0), R = 1, BORE = 0.35;

/** Where `ray` first meets the bead, or where it passes nearest it (Ray and Traced: section 4.2). */
export function bead(ray: Ray): Traced {
  const oc = sub(ray.o, C);
  // Sphere: t^2 + 2 b t + c = 0.
  const b = dot(oc, ray.d), hs = sub(mul(b, b), sub(dot(oc, oc), R * R)), rs = sqrt(max(hs, 0));
  const s0 = sub(neg(b), rs), s1 = add(neg(b), rs);
  // Bore, the infinite cylinder x^2 + z^2 = BORE^2 about C: a t^2 + 2 e t + g = 0 in xz.
  const a = add(mul(ray.d.x, ray.d.x), mul(ray.d.z, ray.d.z)), e = add(mul(oc.x, ray.d.x), mul(oc.z, ray.d.z));
  const hc = sub(mul(e, e), mul(a, sub(add(mul(oc.x, oc.x), mul(oc.z, oc.z)), BORE * BORE))), rc = sqrt(max(hc, 0));
  const c0 = div(sub(neg(e), rc), max(a, 1e-9)), c1 = div(add(neg(e), rc), max(a, 1e-9));
  const inBore = and(ge(hc, 0), and(ge(s0, c0), lt(s0, c1)));      // the sphere's entry falls inside the bore
  const t = select(inBore, c1, s0);                                  // then the first surface is the bore's far wall
  const hit = and(ge(hs, 0), and(ge(t, ray.t0), and(lt(t, s1), lt(t, ray.t1)))); // ...if the wall ends inside the ball
  const p = add(ray.o, mul(ray.d, t)), q0 = sub(p, C);
  const normal = select(inBore, normalize(vec3(neg(q0.x), 0, neg(q0.z))), normalize(q0)); // the bore's faces its axis
  // A miss: the bead's distance where the ray passes nearest its centre (coverage and ink are measured from it).
  const tn = max(neg(b), ray.t0), q = sub(add(ray.o, mul(ray.d, tn)), C);
  const gap = max(sub(length(q), R), sub(BORE, length(vec2(q.x, q.z))));
  return { t: select(hit, t, tn), gap: select(hit, 0, max(gap, 1e-6)), normal };
}

export default scene({
  ground: plane, style: toon(), sun: [-0.4, 0.8, 0.45],
  things: [draw.traced("bead", { bound: bound.sphere([0, 1, 0], 1), hit: bead, paint: () => vec3(0.8, 0.3, 0.2), outline: "silhouette" })],
  cameras: { whole: { kind: "perspective", eye: [2.5, 3.5, 3.5], target: [0, 1, 0], fovY: 0.6 } },
});
```

Loft supplies the bound, the view ray, depth, coverage, ink, the style's light, the outline and the CPU twin. The
author writes only the intersection.

### 1.3 A field that mirrors another

```ts
// A polished ball mirrors an upright ring beside it. The ball's paint sends the reflected ray to the ring's own
// field (exact) and to everything else's stand-in (castRay); the sky colour fills the rest.
import { add, bound, castRay, div, dot, draw, lt, march, mix, mul, plane, saturate, scene, select, sub, toon, vec3, type F, type P3, type Ray, type Traced } from "../src/index.ts";
import { sdSphere, sdTorus } from "../src/lib/index.ts";

const ORANGE = vec3(0.9, 0.45, 0.1), SKY = vec3(0.75, 0.85, 0.95);
const ring = (p: P3) => sdTorus(sub(p, vec3(2.4, 1.2, 0)), 0.9, 0.25);   // in the xy plane: it faces +z
const ringBound = bound.box([1.2, 0, -0.3], [3.6, 2.4, 0.3]);
const ringHit = march(ring, { within: ringBound, steps: 48 });

const mirror = (hit: { p: P3; normal: P3; d: P3 }): P3 => {
  const r = sub(hit.d, mul(hit.normal, mul(2, dot(hit.d, hit.normal))));
  const ray: Ray = { o: add(hit.p, mul(hit.normal, 0.01)), d: r, t0: 0, t1: 30, width: (t) => mul(t, 0.01) };
  const a = ringHit(ray), b = castRay(ray);
  const cover = (h: Traced): F => sub(1, saturate(div(h.gap, ray.width(h.t))));   // 1 on a hit, 0 well clear
  // Nearer first: whichever of the ring and the rest the ray meets sooner covers the other.
  const over = (near: P3, kNear: F, far: P3, kFar: F): P3 => mix(mix(SKY, far, kFar), near, kNear);
  return select(lt(a.t, b.t), over(ORANGE, cover(a), b.colour, cover(b)), over(b.colour, cover(b), ORANGE, cover(a)));
};

export default scene({
  ground: plane, style: toon(), sun: [-0.4, 0.8, 0.45],
  things: [
    draw.field("ring", ring, { bounds: ringBound, paint: () => ORANGE, steps: 48, outline: "silhouette", schedule: "trace" }),
    draw.field("ball", (p) => sdSphere(sub(p, vec3(0, 1, 0)), 1), { bounds: bound.sphere([0, 1, 0], 1.02), paint: mirror, steps: 32, outline: "silhouette", schedule: "trace" }),
  ],
  cameras: { whole: { kind: "perspective", eye: [1, 1.6, 5], target: [1.2, 1, 0], fovY: 0.6 } },
});
```

### 1.4 What a stranger asked, and what changed

| Question on reading cold | Defect? | Change |
|---|---|---|
| "What is `row` in `paint`? The corner's numbers, or something else?" | yes | It is the row **interpolated across the triangle**, as (u, v) is on a surface. Said in the type's comment. Equal numbers at a triangle's three corners (a shard's index) stay exact. |
| "Do I have to compute normals?" | yes, it was optional in the first draft | No. Loft computes them from the rows and turns them with `at` (4.1). Flat shading comes from not sharing corners, which the rim above already does. No option. |
| "Why does the shard's shadow need my own `skeletons`?" | no | The default shadow is a blob fitted to the rest mesh. A blob cannot know that 36 pieces fly apart, and only the author knows which corners belong together. The comment says so. |
| "Is `castRay` the ring too, so the ring appears twice?" | partly | `castRay` meets every thing as its **stand-in**: its skeleton's capsules. A field's stand-in is a blob inside it, so the exact hit comes first wherever both are met. Said in `castRay`'s comment. |
| "Why is my mirror banded by the toon light?" | yes, and not mine to fix | Paint is lit by the style. A mirror that ignores the style needs the materials proposal's wrap (section 8). |
| "What does `width` on a ray do?" | yes, the first draft had `pixel` | It is the ray's width at distance t: a pixel's footprint for a view ray, a light's size times t for a shadow ray. Renamed `width`; section 4.2 explains why one number serves both. |
| "Can the hit of `draw.field` see the view direction?" | yes: `FieldHit` lacked `d` | Every traced paint is told `d`, the ray's direction. That is exact on the CPU too, which `viewDir` (GPU-only) is not. |

---

## 2. What exists, verified in the code

| | There? | How, and where |
|---|---|---|
| **Triangles and quads** | Made by Loft: yes. Made by an author: not supported. Quads: no. | `draw.surface` draws a (u, v) grid. `vs` turns `vertex_index` into a grid corner and evaluates the surface's expressions there (`src/draw/surface.ts`, the `@vertex fn vs` in `SurfaceDrawable.draws`). Two triangles per cell (`quad_corner`), no vertex buffers, per-instance level of detail. A static field's mesh is surface nets of its distance brick (`src/map/nets.ts`, `TriangleMesh`). It is drawn by **vertex pulling** from two static tables, positions plus normals and four indices per vec4 (`src/draw/field.ts`, `meshTable`, `indexTable`, `meshWgsl`), with an inverted-hull outline. Out: `src/bake/` writes glTF from `Bakeable`s, which may be `SurfaceBakeable` or `MeshBakeable` (`src/compile/context.ts`). |
| **Distance-field shapes** | Yes | `draw.field` (`src/draw/field.ts`). The field is sampled on the CPU first, and a measured Lipschitz constant above 1.01 is refused (`FieldSafetyError`). Schedules are `trace`, `brick` (free-space steps by a distance brick, `src/map/brick.ts`), `mesh` and `auto`. Primitives and CSG are in `src/lib/sdf3.ts`, folds in `fold.ts`, unfolds into lists in `similar.ts`. |
| **Ray marching** | Yes, primary rays only | Sphere tracing inside a bound's proxy box (`fragmentWgsl` in field.ts: closest-approach coverage, secant refinement, near-miss ink). A shell march in (u, v, height) with linear search and regula falsi (`src/relief/trace.ts`, written in expressions with `iterate`). A cell walk for lattices (`impostors.ts`). Media are **not** marched: `draw.density` integrates Gaussian blobs in closed form (`src/soft/density.ts`). |
| **Ray tracing** | Primary: yes. Secondary: one, internal. | Exact closed forms for sphere, capsule and rounded cone: `imp_hit` in `impostors.ts`, reused by `draw.copies` (`copies.ts`). One secondary ray: the shadow block (`src/draw/light.ts`) draws one box per skeleton capsule; each fragment asks whether the ray from the opaque point behind it toward the sun passes through that capsule. Water shadows are the same capsules projected along the sun (`shadows.ts`). On the CPU, `staysInView` tests sight lines against the same capsules (`src/check.ts`, `crosses`). Reflection is a post pass that samples 24 pixels up the screen (`src/style/post.ts`, `reflection`). |

What the shared tracer code is (question 2). `src/draw/trace.ts` (268 lines) holds what every tracer needs:

- the bound (`TraceBound`: box, sphere or capsule);
- the proxy box drawn with its front faces culled;
- the analytic span of the ray inside the bound;
- `trace_origin`, `trace_depth` and `trace_mask` (coverage as a 4-sample mask);
- the style's shade, mark and ink at a traced point.

`draw.field` and `impostors` use all of it. `draw.copies` uses the plumbing but has its own per-copy vertex stage.
`draw.relief` uses none of it: its proxy is the shell surface, and it writes depth without a coverage mask. All three
fragment stages end in the same twelve lines: coverage from a gap in pixels, the mask, discard, paint, shade or ink,
depth.

How hard a new primitive is today (question c). Both measured by writing them:

- **An author's own triangles**: 70 lines of TypeScript and WGSL from public exports only (`out/mesh_probe/probe.ts`):
  a `Drawable`, two static `TableSource`s, vertex pulling, and the prelude's 8-argument `shade(base, n, eye, p, depth,
  uv, anchor, id)`, which is not documented anywhere a scene author reads. It draws and it is lit. It has no outline,
  no shadow, no bake and no CPU twin, and the author has to know the prelude's internals.
- **An author's own tracer**: about 200 lines. `traceWgsl`, `tracePlumbingWgsl` and `traceShadeWgsl` are **not
  exported** (`src/core.ts` exports only `bound`, `TraceBound` and `TraceOutline`). An author copies them, then writes
  a fragment stage. Rule 2 says each level is a few lines over the one below; here the step is 200 lines and the
  library's internals. That is a cliff.

One inconsistency found on the way, which the recommendation removes. `field`, `copies` and `impostors` write
`trace_mark(...)` straight into alpha. Surfaces and relief go through `ctx.markOut`, which also records whether a
pixel is outlined for the screen-space ink pass. It appears that traced solids are not marked for `outline: "screen"`
(not tested here).

---

## 3. Options

### 3.1 Triangles from the author's code

| Family | What a user writes | What it cannot express | Cost to the library |
|---|---|---|---|
| **Special types** (three.js's `BufferGeometry` with named attributes, a `Mesh` class, a `Quad` kind) | `new Mesh({ position, normal, uv, index })`, then `mesh.position.needsUpdate = true` to move it | Motion except by mutating buffers, which means state and an order of calls; attributes beyond a fixed menu of names | A class, an update protocol and hidden state. It breaks "any frame draws alone". |
| **Parameters only** | `draw.mesh(name, { points, triangles }, { paint })` with static numbers | Any motion; growth; body coordinates for paint | About 120 lines, but it is a dead end: the first moving shard needs a new feature |
| **Functions only** | `point(i)` and `triangle(k)` as expressions of the index and `time` | Irregular topology from a discrete algorithm (Voronoi, a hull) unless the author re-expresses it as a closed form | About 80 lines |
| **Shaders only** | A raw WGSL vertex stage over the author's tables (possible today: 70 lines) | CPU twin, checks, bake, outline, shadow | Nothing; it exists |
| **Rows and `at`** (recommended) | Rows of numbers per corner and triangles, both made by any TypeScript at build, plus `at(row, i)`, an expression of the row, the index and `time` | Topology that changes per frame beyond a fixed superset; a per-frame JavaScript algorithm (that is state, section 8) | About 200 lines, of which about 60 move out of `field.ts` |

Rows and `at` is the union of the middle three. Parameters-only is `at` left at its default. Functions-only is
`points: n`, with `at` making each point from its index. Shaders-only is still underneath, through `wgslFn` inside
`at`. Its one idea is **split the discrete from the continuous**: the algorithm that decides topology (hard as an
expression, trivial in TypeScript) runs once at build, and the motion that must answer for every frame (hard to keep
deterministic as code, trivial as an expression) runs per vertex. Growth fits as a superset with birth times. The
author's algorithm records when each triangle appears; `at` keeps a triangle's corners at its seed point until then
and grows them out after. A zero-area triangle costs one vertex invocation per corner and no pixels.

### 3.2 A tracer an author writes

| Family | What a user writes | What it cannot express | Cost |
|---|---|---|---|
| **Special types** (`draw.torus`, `draw.csg`, more `kind`s on `copies`) | pick from a list | anything off the list | a closed form per kind; a menu |
| **Parameters only** (a richer `impostors` shape list) | `{ kind: "torus", ... }` | same | same |
| **Shaders only** (export `traceWgsl` and the plumbing) | a WGSL fragment stage over exported helpers | CPU twin; reuse of the hit as a secondary ray; checks | about 10 lines exported; a WGSL contract to keep stable |
| **Functions only** (recommended): `draw.traced(name, { bound, hit, paint, outline })` | `hit(ray) => { t, gap, normal }` as expressions | marches that must stop early, until `iterate` can end a loop (4.4) | about 180 lines; copies and field become compositions |
| A tracer from a field only (today's `draw.field`) | a distance function | exact surfaces with no distance field (intervals, a bilinear patch, a quartic) | exists |

### 3.3 Rays beyond the first

| Option | What it is | Buys | Cost (measured or derived) | Verdict |
|---|---|---|---|---|
| A ray against **an author's own tracer** | call your `hit` (or `march(field)`) from paint with a new `Ray` | exact reflection, refraction and shadow from that thing, with its own paint | the tracer's cost per pixel that asks | **yes**: no new mechanism; it falls out of `draw.traced` |
| A ray against **distance fields** | a second sphere trace | exact shadow and reflection of fields; soft shadow from the closest approach | steps times evaluations. For the most expensive field example, one evaluation over its bound's pixels is about 0.8 ms (DESIGN.md 7.7): 10 to 30 ms for a 32-step second ray. Cheap fields (a torus) cost about a primary march again. | **yes, per field**, as `march(field)`. Never implicitly for all fields. |
| A ray against **the ground** | the plane y = 0 in closed form | shadows and reflection of the floor | a few flops | **yes**, inside `castRay` |
| A ray against **skeleton capsules** (the stand-in) | every thing's chain of capsules, per frame, already on the GPU (`casters`) | soft shadows from any light; reflections of things as blobs in their mean colour; occlusion; the same test the CPU checks use | *measured*, brute force full screen at 1280x800: about 0.045 ms per capsule, so 526 capsules cost 25.6 ms. With two-level culling by chain bounds (48 chains in the largest example): *predicted* 0.3 to 0.6 ms on body pixels | **yes**: `castRay` |
| A ray against **rasterised triangles** | a bounding volume hierarchy over every triangle, rebuilt each frame because everything moves | exact shadows and reflections of surfaces | the largest example has 125k triangles. A per-frame build needs a sort, which Loft's table programs (map and scan) cannot express. *Estimated* 1.5 to 3 ms build plus traversal, against a 2.1 ms frame. | **no** |
| **A distance brick of the whole scene** | the scene's distance on a grid | rays at a tenth of an evaluation per step | per frame: 64³ cells times 526 capsules is 138M capsule evaluations, about 6 ms at the measured rate. Static things once at build: cheap (`brick.ts` exists). | **no per frame**; a static brick is a relative (section 6) |
| **Screen-space** rays against the depth buffer | the existing `reflection` pass, generalised | exact colour of what is on screen | 24 samples per pixel exists today at about 0.35 ms | keep as is; a relative |
| **A general path tracer** | many bounces, many samples | global illumination | needs the triangle hierarchy above, plus many samples or accumulation over frames, which is state | **no** (section 5) |

What each option buys. Soft shadow and occlusion need only *whether, and how nearly*, a ray is blocked. Reflection
and refraction also need *what colour* is there. So:

| | Soft shadow | Occlusion (a few cone rays) | Reflection | Refraction |
|---|---|---|---|---|
| author's tracer, `march(field)` | exact | exact | exact, with that thing's paint | exact through that thing (cast from inside) |
| `castRay` (capsules plus ground) | good: the round chain, not the exact surface | approximate | things as blobs in their mean colour; ground exact | of the background through a thin interface |
| screen space | none | crude | exact where visible on screen | none |
| the sky (materials proposal) | none | none | sky only | sky only |

---

## 4. The recommendation

Four values (`draw.mesh`, `draw.traced`, `march`, `castRay`), two types (`Ray`, `Traced`), and one optional
argument on `iterate`.

### 4.1 `draw.mesh`

```ts
/** Triangles made by the author's code (src/draw/mesh.ts). */
export type MeshData = {
  /**
   * One row of numbers per point, every row the same width (3 to 16): the first three are its rest position unless
   * `at` says otherwise, the rest are the author's own (a body coordinate, a piece's index, a birth time). Or a count,
   * when `at` makes each point from its index alone.
   */
  readonly points: readonly (readonly number[])[] | number;
  /** Three indices into `points` per triangle, counter-clockwise seen from the front (the outside of a solid). */
  readonly triangles: readonly (readonly [number, number, number])[];
};

export type MeshOptions = {
  /** Where point i is now: an expression of its row (one F per number), its index and `time`. Default: row[0..2]. */
  readonly at?: (row: readonly F[], i: F) => P3;
  /**
   * Base colour, lit by the style. `row` is interpolated across the triangle, as (u, v) is on a surface; numbers equal
   * at a triangle's three corners stay exact. `at.copy` is the copy's index under `draw.posed`.
   */
  readonly paint: (row: readonly F[], at: PaintAt) => P3;
  readonly sides: "one" | "two";
  /** `hull`: the triangles pushed out along the welded normal (see below) and drawn back faces only, in ink. */
  readonly outline: "hull" | "none";
  /** What casts shadows. Absent: one blob fitted to the rest points (`fitBlob`), which does not follow `at`. */
  readonly skeletons?: readonly Skeleton[];
};

draw.mesh(name: string, mesh: MeshData, o: MeshOptions): MeshDrawable; // Drawable, Poseable, Instanceable
```

Normals are not an option. At the boundary, `draw.mesh` classifies the mesh once into one of two variants:

```ts
type Normals =
  /** Rows carry a rest position: rest normals at build, carried through `at` in the vertex stage. */
  | { readonly kind: "carried"; readonly rest: Float32Array; readonly welded: Float32Array }
  /** `points` is a count, or `at` does not read row[0..2]: the moved triangle's own face, per pixel. */
  | { readonly kind: "faces" };
```

- **carried.** At build, each point's rest normal is the area-weighted sum of the normals of the triangles that
  share its index. `welded` is the same sum over triangles that share its **position**, so a hull outline stays
  closed across the hard edges of an unshared mesh. In the vertex stage both are carried through `at` by the
  cofactor of its Jacobian with respect to row[0..2]:
  `n' ∝ n.x (∂at/∂y × ∂at/∂z) + n.y (∂at/∂z × ∂at/∂x) + n.z (∂at/∂x × ∂at/∂y)`.
  DERIVATION: tangents map by J, so normals map by cof(J) = det(J) J^-T. Checks: J = I gives n; a rotation R gives
  R n. The Jacobian is symbolic (`jacobian`, `src/grad.ts`). A GPU-only function inside `at` cannot be
  differentiated, so it is refused by name and the fix is given (use a count form, or compute the motion in
  expressions).
- **faces.** `normalize(cross(dpdx(world), dpdy(world)))` in the fragment stage: exact and flat for any motion. Flat
  shading of a rows mesh comes from not sharing corners, as the pane's rim does.

Where it plugs in:

- **`src/draw/mesh.ts`** (new, about 200 lines). `meshTable`, `indexTable` and `meshWgsl` move here from `field.ts`.
  `field.ts`'s mesh schedule becomes `draw.mesh(name, fromNets(mesh), { paint })`. Its pixels are byte-identical: the
  same tables and the same WGSL, with `at` the identity, which the vertex stage elides.
- **Tables.** The rows go in one static table (ceil(width / 4) vec4 lanes per point, plus two for the normals) and the
  triangles in another (four indices per vec4, exact as f32 below 2^24 points, which is already checked).
- **Vertex stage.** It reads a corner's index, then its row, then evaluates `at` (emitted with `emitShaderFn`, the row
  bound to variables), carries the normals, and passes the row as up to four `vec4` varyings. WebGPU guarantees 16
  inter-stage locations; a row of 16 numbers uses 4 and world, normal and anchor use 3.
- **Bake.** `MeshBakeable.paint` gains the row argument. `bake/mesh.ts` evaluates `at` and `paint` with `compileJs`
  at the bake's time. The nets mesh passes an empty row.
- **CPU twin.** `at` and `paint` are expressions, so `compileJs` evaluates any point at any time. `budget({
  triangles })` counts the mesh's triangles.
- **Copies.** `posed(name, poses)` draws one instance per pose, as `SurfaceDrawable.posed` does.
- **Level of detail.** None. One mesh is drawn at every distance. Section 6 states the budget instead.

### 4.2 `Ray`, `Traced`, `draw.traced`

```ts
/**
 * A ray, or a thin cone: from `o` along unit `d`, searched for t in [t0, t1]. `width(t)` is how wide the ray is at t.
 * For a view ray it is the pixel's footprint; for a shadow ray, the light's apparent size times t; for a reflection,
 * how much it blurs.
 */
export type Ray = { readonly o: P3; readonly d: P3; readonly t0: F; readonly t1: F; readonly width: (t: F) => F };

/**
 * Where a ray meets a solid, or passes nearest it. `gap` is the solid's distance at t: 0 on a hit, positive on a miss,
 * at the t where gap / width(t) is least. `normal` is the solid's outward normal there.
 */
export type Traced = { readonly t: F; readonly gap: F; readonly normal: P3 };

export type TracedOptions = {
  /** Where the hit is searched: one static bound, or one box per copy (expressions: copies may move). */
  readonly bound: TraceBound | { readonly count: number; readonly box: (copy: F) => { readonly centre: P3; readonly half: P3 } };
  /** The intersection, as expressions: the ray spans the bound (or the copy's box). */
  readonly hit: (ray: Ray, copy: F) => Traced;
  /** Base colour at a hit, lit by the style: the point, normal, ray direction and copy index. */
  readonly paint: (hit: { readonly p: P3; readonly normal: P3; readonly d: P3; readonly copy: F }) => P3;
  readonly outline: TraceOutline;
  /** Absent: a blob fitted by casting the hit on the CPU through the bound (a static bound), or none (copies). */
  readonly skeletons?: readonly Skeleton[];
};

draw.traced(name: string, o: TracedOptions): TracedDrawable;
```

**Why one `gap` serves coverage, ink and soft shadow** (mine, built on inherited pieces: cone tracing, Amanatides
1984; SDF soft shadows, Quilez 2010). Every consumer asks the same question: what share of a cone of width w(t) does
the solid cover? Within one pixel it is `1 - gap / width`, which is today's `cover = clamp(1 + edge - gapPx)` in all
three tracers. Ink is a near miss, `gap / width < ink px`. A shadow ray whose width is the light's size times t gives
the classic penumbra `gap / (k t)`. So a tracer written once for drawing is already a correct soft-shadow ray. The
march keeps the t where `gap / width` is least, which is what `field.ts` already tracks (`nearPx`).

Where it plugs in:

- **`src/draw/traced.ts`** (new, about 180 lines). It contains:
  - the proxy (`traceWgsl` for one bound; the per-copy box vertex stage from `copies.ts`, moved);
  - the fragment template, which builds the view `Ray` from `trace_origin`, `bound_span` and `pixel_world`, calls the
    emitted `hit`, and runs the twelve shared lines;
  - shading through `ctx.markOut`, which also fixes the screen-outline mark noted in section 2.
- **`draw.copies` becomes a composition**: `draw.traced(name, { bound: { count, box: copyBox(table) }, hit:
  (ray, k) => capsuleHit(ray, fetch(table, k, …)), paint, outline })`. Its `inkBack` depth offset becomes one field on
  `Traced` only if the port measures that it is needed. Otherwise it is dropped.
- **`draw.field` becomes a composition over `march`** (4.3), with its schedules kept. `impostors`' grid walk becomes
  its `hit`. `draw.relief` stays separate: its proxy is a surface's shell, not a box. A third bound variant would be
  needed, and nothing asks for it yet.
- **CPU twin.** `hit` is an expression, so `compileJs` casts rays on the CPU. The default blob shadow casts a grid of
  rays through the bound and fits the hits with `fitBlob`, which already exists. A check can ask "does this ray hit
  the bead" with no render.

### 4.3 `march`

```ts
/**
 * A field's sphere trace as a hit function: steps by the field's distance from the ray's t0, clipped to `within`,
 * stopping at a hit (distance below a share of the ray's width), at t1, or after `steps` evaluations. Checked like
 * draw.field: sampled inside `within` on the CPU and refused if it can overshoot, unless `safety` is given.
 */
export function march(field: Field, o: { readonly within: TraceBound; readonly steps: number; readonly safety?: number }): (ray: Ray) => Traced;
```

This is `fragmentWgsl` of `field.ts` written as expressions:

- the closest-approach estimate between two samples;
- the near miss;
- the exhausted-march rule;
- the two secant steps;
- the normal by four differences at the hit.

The brick's free-space skip stays hand-written WGSL, called through a `LibFn`. It is GPU-only, so a march with a
brick has a CPU twin only without the brick, which gives the same hits a step at a time.

### 4.4 `iterate` that can end

```ts
iterate(count, init, step, until?: (state) => Expr<"bool">)
```

When given, `until` is tested after each step. True ends the loop: a `break` in WGSL and in the JavaScript twin.
Without it, a march costs all its steps on every pixel. DESIGN.md 7.7 records that the step cap "changes almost
nothing" because rays end well before it, so the saving is the gap between the cap and the mean. About 30 lines in
`src/expr.ts` and the two emitters. No new name.

### 4.5 `castRay`

```ts
/**
 * A ray against every thing's stand-in, the capsules of its skeleton as it is posed this frame, and against a plane
 * ground (y = 0). Returns the nearest hit, or the nearest miss weighed by the ray's width (Traced), and the colour
 * there: a thing's mean paint (sampled once on the CPU at build), or the style's ground colour.
 */
export function castRay(ray: Ray): Traced & { readonly colour: P3 };
```

Where it plugs in:

- **A scene function**, as `sea_height` is. The expression is a call to `loft_cast`, a `LibFn`. When a scene's
  compiled expressions call it, the compiler writes its WGSL from `casters(ctx.things)` (the joint table the shadows
  already fill on the GPU every frame, `src/draw/shadows.ts`) and adds one map table: one bounding sphere per chain.
  The WGSL tests the ray against the chains' spheres, then against the capsules of the chains it meets, plus the
  plane.
- **Nothing else changes.** A scene that never calls `castRay` builds neither the table nor the function, so its
  artefact is byte-identical.
- **CPU twin.** It is `staysInView`'s capsule test (`crosses` in `src/check.ts`) generalised to return t, gap and
  colour. `staysInView` is then rewritten on `castRay`: the second user, from an unrelated module, before anything is
  called settled (rule 9).
- **Colour.** Each chain records the thing it came from. At build the thing's paint is evaluated on the CPU over its
  bakeable's grid at t = 0 and averaged into one colour per thing. A thing with no bakeable gets the style's ink.
- **The sea** is met as its still plane. Its swell is not traced (section 6).

Nothing else changes for existing scenes. No existing constructor's signature changes, except that `FieldHit` gains
`d` and `copy`, which existing paints ignore.

---

## 5. What stays out, and why

- **File import.** A mesh from a modelling tool has no body coordinates, no parameters and no closed form, so it
  loses paint in its own coordinates, checks and variants: everything Loft is for. three.js serves assets better
  (`docs/vs_threejs.md`). An author **can** parse a file in their own module and pass rows, because `points` are
  numbers from anywhere. Loft neither stops that nor helps with it. No loader, no format, no material import.
- **A general path tracer.** It needs a hierarchy over moving triangles, rebuilt each frame. That build is a sort,
  which Loft's GPU table programs (map, scan) cannot express, and on this scene it costs about as much as the whole
  frame. It also needs many samples per pixel: either noise, or accumulation across frames, which is state and
  breaks "any frame draws alone". A stylised look has no use for converged global illumination; a style decides
  light.
- **Rays against arbitrary rasterised surfaces.** For the reason above. Surfaces cast and reflect as their skeletons.
- **Quads.** A quad is two triangles and a choice of diagonal. An algorithm that makes quads (Catmull-Clark) splits
  them. Exact bilinear patches are a relative of `draw.traced` (section 6), not a mesh primitive.
- **Implicit secondary rays.** Loft never adds a reflection or shadow ray on its own. A ray is something an author's
  paint, light or style part calls, so its cost is written where it is paid.

---

## 6. Limits and relatives, as the components' comments will give them

**`draw.mesh`.**

- *Law.* Triangles from rows the author made at build, each point moved by `at`, an expression of its row, index and
  time. Normals at rest are carried through `at` by its Jacobian.
- *Cannot.*
  - Change topology per frame, beyond a superset whose triangles collapse until wanted.
  - Run a per-frame algorithm (a step function is state: bake it as a table, then read it in `at`).
  - Draw fewer triangles far away (no level of detail).
  - Make a shadow that follows `at`, unless given skeletons.
  - Differentiate a GPU-only function inside `at`.
  - Keep hard edges where corners are shared (unshare them).
- *Budget.* *Measured*: about 2.7 ms per million triangles at 1280x800 (vertex pulling, three vertex invocations per
  triangle). 100k triangles is about 0.27 ms. Static tables are 4 to 24 floats per point.
- *Going past it.* `at` may `fetch` any table (a recurrence baked over time, a host's per-frame data). For a mesh that
  is a function of (u, v), use `draw.surface`. For a field's own mesh, use `draw.field` with the `mesh` schedule.
- *Relatives worth trying:*
  - a convex hull or a Delaunay triangulation (any TypeScript library at build);
  - Loop or Catmull-Clark subdivision, split to triangles;
  - growth replayed by birth times;
  - a mesh that is a closed form of its index (a geodesic sphere, a polyhedron's net unfolding over time).

**`draw.traced`.**

- *Law.* The author's `hit` is asked once per covered pixel of the bound's proxy (or of each copy's box). Loft
  supplies depth, coverage, ink, the style's light and the CPU twin from `{ t, gap, normal }`.
- *Cannot.*
  - Antialias a hit function that reports no gap on a miss (edges stair-step).
  - Be baked to glTF (no surface to sample).
  - Trace inside a shell over a surface (that is `draw.relief`).
  - Draw a solid that reaches outside its bound (it is cut there).
- *Going past it.* Raw WGSL inside `hit` through `wgslFn`, which loses the CPU twin.
- *Relatives worth trying:*
  - interval CSG (the bead), after Roth 1982;
  - an exact bilinear patch (Reshetov 2019);
  - a quartic (a torus, solved in closed form);
  - a heightfield walked cell by cell;
  - a lattice by a 3D DDA (`impostors`' walk);
  - a march with your own step bound.

**`march`.**

- *Law.* Sphere tracing a field that the CPU check finds 1-Lipschitz.
- *Cannot.* March a field that overshoots without `safety`; stop before `steps` on grazing rays (it takes the point
  reached); be cheap when the field is expensive: a second ray costs another march.
- *Relatives worth trying:* over-relaxed steps (Keinert et al. 2014, already in the brick skip); segment tracing for
  fields with a known Lipschitz bound per region.

**`castRay`.**

- *Law.* The nearest of every thing's skeleton capsules this frame and a plane ground, with closest-approach soft
  edges and each thing's mean colour.
- *Cannot.*
  - See exact surfaces: a wide flat part (a carapace, a sail) is its round chain.
  - See things with no skeleton.
  - See the sea's swell.
  - Return a hit's real paint.
- *Cost.* About 0.045 ms per capsule tested per million rays. Chains are culled by bounding sphere first.
- *Relatives worth trying:*
  - an author's own tracer or `march` for the one thing that must be exact;
  - closed-form occlusion of a sphere or capsule for ambient light (Quilez), as a light's occlusion term;
  - a static distance brick of the scene's still parts, baked once;
  - screen-space hits for on-screen colour.

---

## 7. The line in `loft context`

The API map is generated, so these entries appear there with no prose (22 words):

```
draw: … draw.mesh(name, {points, triangles}, {at?(row, i), paint(row, at), sides, outline, skeletons?}); draw.traced(name, {bound, hit(ray, copy), paint(hit), outline, skeletons?})
field: fieldPixel; march(field(p), {within, steps, safety?})
trace: bound.box(min, max); bound.sphere(centre, radius); bound.capsule(a, b, radius); castRay(ray)
```

The first part of `context.md` says today "bodies mirror nothing; no simulation; meshes come out (the bake command
writes glTF), none go in". That changes to (two sentences, 64 words):

> Triangles your own code makes (a hull, a shatter, a growth) draw with `draw.mesh`: a row of numbers per point,
> made at build, moved by `at`, an expression of the row and `time`. A solid you can intersect draws with
> `draw.traced`; its `hit`, or `march(field)`, also answers a reflected or shadow ray from any paint, and `castRay`
> meets everything else as its skeleton's capsules.

The craft part gains one clause under "Range in shape", in the topology list, after `impostors` (5 words): "a
mesh of your own `draw.mesh`". The total is 91 words against the 2,200-word budget, of which the old clause it replaces was 9.

---

## 8. How it meets the other three proposals

**Lights** (a light is a function of a world point, with optional occlusion).

- *I give* `castRay` and the `Traced` contract. A light's occlusion is one line:
  `mul(colour, saturate(div(c.gap, ray.width(c.t))))` with `c = castRay({ o: p, d: toward, t0: 0.02, t1: dist,
  width: (t) => mul(t, size) })`. That is a soft shadow sized by the light. For one exact occluder, the author's own
  `hit` or `march` does the same.
- *I need* the light to be evaluated in the fragment stage with the world point, and to reach `draw.mesh` and
  `draw.traced` through the same `shade` entry every body uses. They call `shade` exactly as surfaces do, so they get
  lights for free.
- *Cost to share:* one cast per light per pixel that asks, about 0.3 to 0.6 ms on the largest example's bodies. The
  sun keeps its splatted shadow block, which costs only what it covers and is cheaper.

**Materials** (paint channels and a wrap; a sky gloss).

- *I need* paint in `draw.mesh` and `draw.traced` to return whatever paint returns elsewhere, so channels apply
  unchanged; I add no material concept. I also need the wrap to see the view direction and the normal, so a mirror
  can stop being lit by the style (scene 1.3's question).
- *I give* the reflection beyond the sky. A gloss part can call `castRay(reflected)` and blend `colour` by coverage
  over `sky(r)`, so a body gains the dark shapes of its neighbours in its sheen, not only the sky.

**State** (a step function baked into a table over the scene's period).

- *I need* nothing new: `at` reads the baked table with `fetch(table, i + count * frameIndex(time), lane)`, which
  makes cloth, accretion or a crowd of points a `draw.mesh`.
- *I give* the CPU twin of `castRay`. A step function run on the CPU at build can collide its particles with the
  scene's stand-ins, the same capsules that draw the shadows, deterministically.

---

## 9. Predictions a build will be scored against

Machine and method as DESIGN.md 7.7: GPU timestamps, best of 30 to 40 frames, fast and slow clock states reported
separately. "Largest example" is `examples/bosses/reefback_titan.ts` from `game` at 1280x800: 2.1 ms (fast state) as
given, and 5.44 ms *measured* today in the slow state, with 117 draws, 125k triangles, 48 shadow chains and 526
capsules.

| Piece | Prediction |
|---|---|
| Existing scenes, through every stage | Byte-identical pictures and frame hashes, except the two ports, which are gated below. `field.ts`'s mesh schedule is byte-identical after the move. |
| `draw.mesh`, shatter scene (36 shards, about 900 triangles) | Below the timer's tick (under 0.07 ms). Shader module 2 to 4 KB (the probe's was 1.4 KB). Build under 20 ms. |
| `draw.mesh`, 100k triangles | +0.27 ms (±0.1) at 1280x800. Build under 150 ms (normals, weld, tables). Tables under 10 MB. |
| `draw.mesh`, 1M triangles | 2.6 ms (±0.6) *measured* on the probe: the stated budget. |
| `draw.traced`, bead filling a third of the screen | Under 0.3 ms. Module about 9 KB (`copies`' is 11 KB). |
| `copies` ported onto `traced` | The list example's draw within 5% of today's (1.38 ms, three view; 3.08 close). Under 0.1% of pixels differ by more than 8/255. |
| `field` ported onto `march` | The bricked field example within 10% of today's (4.5 to 4.9 ms overview; 22.6 to 25.2 close); the nested and pearl examples within 10%. |
| `until` on the field march | Field evaluations per SIMD group unchanged against today's hand-written `break` (counted on the GPU as DESIGN.md did). |
| `castRay`, one shadow ray per body pixel, largest example | +0.3 to 0.6 ms in the fast state. About 3 KB added to each module that calls it. |
| `castRay` without chain culling (control) | About 25 ms full screen (*measured* 25.6 brute force at 526 capsules). This is why culling is part of the design. |
| Lines of library code | About +560 added and −250 removed (moves and the two ports): net +310 (±100). |
| Exported names added | **6**: `draw.mesh`, `draw.traced`, `march`, `castRay`, `Ray`, `Traced`; plus one optional argument. |

---

## 10. Staged build plan

Each stage ends on a picture, a measured number against section 9, and the full suite.

1. **`draw.mesh`, static** (1.5 days). Move the mesh path out of `field.ts`; rows, triangles, paint over rows,
   carried normals with `at` the identity, welded hull, blob shadow, bakeable. Picture: the pane, whole, outlined, with
   its shadow. Gate: `field.ts`'s mesh schedule byte-identical.
2. **`at` and copies** (1 day). Jacobian-carried normals; the faces variant; `posed`. Picture: the pane shattering,
   six frames across 0 to 3 s.
3. **`iterate` until, `Ray`, `Traced`, `draw.traced`** (2.5 days). The bead. Then port `draw.copies` onto it. Gate:
   the copies prediction.
4. **`march`, and `draw.field` over it** (2 days). Then the field gates, and the mirror ball in scene 1.3 with
   `march` only.
5. **`castRay`** (2 days). The chain-bounds table, the scene function, the CPU twin, `staysInView` rewritten on it,
   mean colours. Scene 1.3 whole, and a lamp's soft shadow on the largest example as the cost test.

Total about 9 days. The first stage is the smallest thing that draws a picture: an author's triangles, lit, outlined,
shadowed.

**Stop rules.**

- If stage 3's port of `copies` misses its gate by more than 10%, the "compositions" half is wrong, not the drawable.
  Keep `draw.traced` for new tracers, leave `copies` hand-written, and record why.
- If stage 4's expression march is more than 10% slower than the hand-written one after one round of fixes, `march`
  keeps a hand-written WGSL body behind a `LibFn` (its CPU twin is the expression version, as the brick's is).
  `draw.field` keeps its code.
- If `castRay` costs more than 1 ms on the largest example's bodies after chain culling, try one per-frame grid of
  capsule lists (a map table over cells). If that still costs more than 1 ms, `castRay` takes an explicit list of
  things to meet and never the whole scene by default.
- **The design as a whole is abandoned** if Jacobian-carried normals visibly mis-shade the shatter's rigid pieces,
  which would mean the derivation or `grad.ts` is wrong for `rotateAbout`. The fallback is faces normals only, and
  then "rows and `at`" loses smooth moving meshes, its main reason to exist.

---

## 11. Open questions for the owner

1. **Hardware index buffers in the artefact format?** Vertex pulling runs the vertex stage three times per triangle.
   An index buffer would let the GPU reuse transformed corners (up to about 2x on vertex-bound meshes), but it
   changes the manifest and the Rust loader. *Recommended:* not now. 1M triangles at 2.6 ms is enough for meshes made
   by code; revisit at the first mesh over 500k.
2. **Is `march` exported, or only reached through `draw.field`?** *Recommended:* exported. "A field that mirrors
   another" is impossible without it, and it is the mechanism `draw.field` is the component over (rule 2).
3. **Does `castRay` meet every thing by default, or only a list the author passes?** *Recommended:* every thing, as
   shadows do. Add a list only if the cost stop rule fires.
4. **`castRay`'s colour: a thing's mean paint, or its paint evaluated at the hit capsule's body coordinate (a strand
   knows m along its chain)?** *Recommended:* the mean for now. The body coordinate is the first relative to try if
   reflections of creatures read as flat blobs.
5. **Should `draw.relief` also become a composition over `draw.traced`,** with a third bound variant for a surface's
   shell? *Recommended:* no, until a second shell tracer exists. One user does not justify the variant.
