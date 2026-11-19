# Loft language card

Loft is Lua-like, parsed by Rust to a typed graph. A `kind` is code; its columns are a row. Top-level code runs at
build; queries run on the GPU. Types are inferred except on `pub` items and a law's or check's first parameter.

## Spaces and units

```
space World Body Leg Claw Face Shell
local w = pose * {0, 0.2, 0.9}@Claw     -- pose: xf@Claw>World
```

`p3 v3 d3 n3` (point, vector, direction, normal) `@S`; `xf@A>B`; `f u i bool rgb`; `K` a row id, `[K]` a
table. `{x, y, z}` takes space from context. Point + point, mixed spaces: E104. WGSL maths, plus `and or not`.
`0.3m 1.2rad 0.5s 40deg`.

## Kinds and rows

```
kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, foot: p3@World)
  trace(p) = capsule(p, {0, 0, 0}, {femur, 0, 0}, 0.07m)
end
kind Claw[serrated: bool] @Claw (open: rad) ... end

fn crab(b, body)
  for j in 0..6, side in {-1, 1}
    CrabLeg{parent = b, pose = body * socket(j, side), femur = 0.40m + 0.05m * j,
            foot = body * {side, 0, 0.3 * j}@Body}
  end
end
local at = place({0, 0.4, 0}@World)
crab(CrabBody{pose = at}, at)
```

Columns are in scope. `[..]` is a code variant (`[len: m]` E401; over 4, E402). At build `K{..}` appends a row,
returning its id, and `for` unrolls; in a query it loops on GPU.

## Queries

`trace(p)` sphere-traced, `surface(u, v) grid 64x32` rasterised, `visible(ro, rd)` hit t or -1, `material light
sky medium`; `time` in scope. Bounds are derived. E211: zero divisor. `f'(u)` needs C1 (E307).

```
scan whorl u in 0..1 by 256 on bake_lut
  init = frame(spine(u), dir(spine'(u)), {0, 1, 0})
  step prev = transport(prev, spine(u), dir(spine'(u)))
end
-- whorl(u): C2 sample .o .n .b
```

## Domain maps

```
local sym = q |> mirror_x |> move({-0.15, -0.72, -0.42})
local eye_c = {0.03, 0.30, 0.07}@sym
local eye_w = pose * sym.out(eye_c)
local look = sym.into(inv(pose) * dir(gaze - eye_w))
```

`a |> f(x)` is `f(a, x)`. A map chain is a space; `sym.cell` its branch. Also `rot_x rot_z repeat_polar`.

## Materials

```
material(p, n) = ggx(albedo, 0.35 + 0.1 * fbm2(p.xz * 40), 0) + sss({0.9, 0.3, 0.1}, 0.08) + coat(0.7, 0.07)
lobe ink(dark, cut, n, v, l) = if dot(n, l) < cut then dark else {0.92, 0.85, 0.7} end
```

Lobes sum bottom first: `ggx coat film sss emit`, `lobe`. Also `box round_box smin ssub`.

## Laws and checks

```
law crab_step(leg: CrabLeg, dt, bodies: [CrabBody])
  local b = bodies[leg.parent]
  local cp = b.com + b.vel * 0.28s
  if dist(leg.foot, cp) > 0.5m then leg.foot = {cp.x, 0, cp.z} end
end
```

A law runs per row of its first parameter's kind and alone writes it.

```
check upright(b: CrabBody)
  if not finite(b.pose) then return unknown(NonFinitePose) end
  local margin = (b.pose * b.head).y - 0.15m
  if margin < 0 then missed(-margin) else held(margin) end
end
```

## Schedule

```
schedule Main      -- in order
  crabs()
  run_laws(body_law, crab_step)
  check(upright)
  orbit({0, 0.4, 0}@World, 6m, 40deg)
  build_indirect(Ground, CrabLeg, Claw)
  render_bundle(Vis64) do
    draw_indirect(Ground, CrabLeg, Claw)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
end
```

## Raw WGSL

```
pub fn hash(p: v3) -> f = wgsl[[ return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5); ]]
```

Opaque, bound `Unbounded`.

## Parse rules

1. Newline ends a statement unless the line ends in an operator, `,` `(` `{` `[`, or the next starts `|>`.
2. `space kind fn lobe law check scan schedule pub` open declarations; the rest is build.
3. A header ends at its newline; `end` closes it; `=` gives a one-expression body. A last expression is the value.
4. `then` makes a one-line `if`, also an expression. `do` only opens a schedule call's block.
5. `local x = e` binds; `x = e` sets a local; `a.f = e` only in a law.
6. Braces never open blocks: `{a, b}` vector, `K{f = e}` row.
7. Types follow only `:` or `->`, where `>` joins spaces; `@` attaches a space.
8. `0.5m`: digits touching letters are a unit.
9. `--` comments; `'` marks a derivative; no strings.
10. Values lowercase, kinds capitalised; `wgsl[[` is raw to `]]`.
