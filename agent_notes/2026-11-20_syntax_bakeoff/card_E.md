# Loft language card

A `kind` is code, compiled once to WGSL; its parameters are a row, one instance's data. Module-level code runs at
build; queries run on the GPU. Types are inferred except on `pub` items and a law's or check's first parameter. A
block's value is its last expression.

## Spaces

```
space World Body Leg Claw Face Shell
p = [0,0.2,0.9]@Claw     # point, metres
w = pose * p               # pose: xf@Claw>World
```

Types: `p3 v3 d3 n3` (point, vector, direction, normal) `@S`, `xf@A>B`, `f u i bool rgb m rad s kg`, row id `K`,
table `[K]`. `[x,y,z]` takes space and type from context; add `@S` where the space changes. Numbers promote.
Point - point is a vector; point + point or mixed spaces: E104. `inv(x)` inverts.

## Kinds

```
kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, foot: p3@World) {
  trace(p) = capsule(p, [0,0,0], [femur,0,0], 0.07m)
}
kind Claw[serrated: bool] @Claw (pose, open: rad) { ... }
```

`kind K[len: m]` is E401; over 4 used variants, E402.

## Build

```
fn crab(b, body: xf@Body>World) {
  for j in 0..6, side in [-1, 1] {
    CrabLeg(parent: b, pose: body * socket(j, side), femur: 0.40m + 0.05m * j, foot: body * [side, 0, 0.3 * j]@Body)
  }
}
at = place([0,0.4,0]@World)
crab(CrabBody(pose: at), at)
```

At build `K(..)` appends a row and returns its id; `for` unrolls. In a query `for` is a GPU loop.

## Queries

`trace(p)` sphere-traced, `surface(u, v) grid 64x32` rasterised, `visible(ro, rd)` hit t or -1, `material(p, n)`,
`light`, `sky`, `medium`; `time` is in scope. Bounds are derived; a divisor reaching zero is E211.
`f'(u)` needs C1; of `min`, `abs`, Voronoi: E307.

```
scan whorl u in 0..1 by 256 on bake_lut {
  init = frame(spine(u), dir(spine'(u)), [0,1,0])
  step prev = transport(prev, spine(u), dir(spine'(u)))
}
# whorl(u) samples C2: .o .n .b
```

## Maps

```
sym = q |> mirror_x |> move([-0.15,-0.72,-0.42])
eye_c = [0.03,0.30,0.07]@sym
globe = sphere(sym.p - eye_c, 0.085m)
eye_w = pose * sym.out(eye_c)
look = sym.in(inv(pose) * dir(gaze - eye_w))
```

`a |> f(x)` is `f(a, x)`; a bound map chain is a fresh space; `sym.cell` is the branch. Also `rot_x(a) rot_z(a)
repeat_polar(7)`.

## Materials

```
material(p, n) = ggx(albedo, 0.35, 0) + sss([0.9,0.3,0.1], 0.08) + coat(0.7, 0.07)
lobe ink(dark, cut, n, v, l) = if dot(n, l) < cut { dark } else { [0.92,0.85,0.7] }
```

Lobes sum bottom first: `ggx(albedo, rough, metal) coat(w, rough) film(w, ior, nm) sss(color, radius) emit(color)`,
any `lobe`. Library: `sphere box round_box capsule smin ssub fbm2 ray_capsule hit_min`.

## Law

```
law crab_step(leg: CrabLeg, dt, bodies: [CrabBody]) {
  b = bodies[leg.parent]
  cp = b.com + b.vel * 0.28s
  if dist(leg.foot, cp) > 0.5m { leg.foot = [cp.x,0,cp.z] }
}
```

Runs per row of its first parameter, which only it writes.

## Check

```
check upright(b: CrabBody) {
  if !finite(b.pose) { return unknown(NonFinitePose) }
  margin = (b.pose * b.head).y - 0.15m
  if margin < 0 { missed(-margin) } else { held(margin) }
}
```

## Schedule

```
schedule Main {        # a pass per line
  crabs()
  run_laws(body_law, crab_step)
  check(upright)
  orbit([0,0.4,0]@World, 6m, 40deg)
  build_indirect(Ground, CrabLeg, Claw)
  render_bundle(Vis64) { draw_indirect(Ground, CrabLeg, Claw) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
}
```

## Raw WGSL

```
fn hash(p: v3) -> f = wgsl { return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5); }
```

Opaque: no derivative, bound `Unbounded`.

## Parsing

1. Newline ends a statement unless the line ends in an operator or opener, or the next starts `|>`.
2. Declarations open with a keyword; other module lines are build code.
3. Types follow only `:` or `->`; there `>` joins spaces.
4. `@` only attaches a space: `[..]@S`, `T@S`, `kind K @S`.
5. A number touching letters takes a unit or is a lex error.
6. `'` only marks a derivative.
7. Values are lowercase; `K(` constructs a kind or tag, `K[..](` a variant.
8. `{` ends a header; only schedules have `call(..) {` blocks.
9. `x = e` binds a new name or sets a `var`; `a.f = e` only in a law.
10. `wgsl {` is raw text to its balanced `}`.
