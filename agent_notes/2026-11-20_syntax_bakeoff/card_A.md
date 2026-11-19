# Loft language card

Loft is WGSL plus a few declarations. `comptime` code runs at build and emits rows. Each `kind` compiles once; the GPU
runs it over its rows. A kind is code; a row is one instance's data.

## Spaces and units

```
space World, Body, Leg, Claw, Face;
let p = Pt[Claw](0.0, 0.2, 0.9);           // components are metres
let w: Pt[World] = self.pose * p;   // pose: Xform[Claw -> World]
```

Pt - Pt is a Vec; Pt + Pt is an error; mixing spaces is E104. `Dir[S]` and `Norm[S]` have unit
length; normals map by inverse transpose. `inv(x)` inverts an Xform. Units: `Meters Rad Sec Kg`; suffixes `0.3m 1.2rad 0.5s 40deg`.

## Kind and row

```
kind CrabLeg {
  row { parent: RowId[CrabBody], pose: Xform[Leg -> World], femur: Meters, foot: Pt[World] }
  query trace(self, p: Pt[Leg]) -> Field[Leg] {
    return sd_capsule(p, Pt[Leg](0.0, 0.0, 0.0), Pt[Leg](self.femur, 0.0, 0.0), 0.07m);
  }
}
kind Claw[comptime serrated: bool] { ... }   // discrete only
```

Per-instance values go in `row`. `kind K[comptime len: Meters]` is E401; over 4 used variants is E402.

## Comptime: emitting rows

```
comptime fn crab(b: RowId[CrabBody], body: Xform[Body -> World]) {
  comptime for j in 0u..6u {
    comptime for side in [-1.0, 1.0] {
      emit CrabLeg { parent: b, pose: body * leg_socket(j, side), femur: 0.40m + 0.05m * f32(j),
                     foot: body * Pt[Body](side, 0.0, 0.3 * f32(j)) };
    }
  }
}
```

`emit` appends a row and returns its `RowId`, never a shader. A `for` inside a kind is a GPU loop.

## Queries

A kind implements any of: `trace(self, p) -> Field[S]` (sphere-traced distance); `@grid(nu, nv) query surface(self,
u, v) -> Pt[S]` (rasterised patch); `visible(self, ro, rd) -> f32` (closed-form hit t, -1 on miss); `material`,
`light`, `sky`, `medium`. Any query may take `time: Sec`.

The compiler derives every `Field` bound. A divisor that can reach zero is E211: clamp it.
`diff(f, u)` needs C1; `diff` of `min`, `abs` or Voronoi is E307.

```
scan whorl(u: f32 in 0.0..1.0, steps: 256u) @schedule(bake_lut) -> Table[Frame[Shell]] {
  init: frame_from_tangent(spine(u), normalize(diff(spine, u)(u)), Dir[Shell](0.0, 1.0, 0.0));
  step(prev, u_i): transport_frame(prev, spine(u_i), normalize(diff(spine, u)(u_i)));
}
// let f = sample_c2(whorl, u);  f.origin, f.normal, f.binormal
```

## Domain maps

```
map sym = mirror_x(q) |> translate(-Vec[Face](0.15, 0.72, 0.42));
let eye_c = Pt['sym](0.03, 0.30, 0.07);
let globe = sd_sphere(sym - eye_c, 0.085m);
let eye_w = self.pose * (sym.to_parent * eye_c);
let look  = sym.from_parent * (inv(self.pose) * normalize(self.gaze - eye_w));
```

`map` makes a fresh space `'sym`; `sym.cell` is the branch (mirror side, sector). Also `rot_x`, `rot_z`, `repeat_polar(p, 7u)`.

## Materials

```
query material(self, p: Pt[Leg], n: Norm[Leg]) -> Bsdf {
  return ggx(albedo, 0.35, 0.0) + sss(vec3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07);
}
@lobe fn ink(dark: vec3<f32>, cut: f32, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec4<f32> { // a: passed below
  return vec4(select(vec3(0.92, 0.85, 0.7), dark, dot(n, l) < cut), 0.0);
}
```

A material sums lobes, bottom first: `ggx(albedo, rough, metal)`, `clearcoat(weight, rough)`,
`thin_film(weight, ior, nm)`, `sss(color, radius)`, `emission(color)`, or any `@lobe` fn. 
Library: `sd_sphere sd_box sd_round_box sd_capsule smin_poly ssub_poly fbm2 ray_capsule hit_min`.

## Motion law

```
law crab_step(leg: CrabLeg, dt: Sec, bodies: Table[CrabBody]) writes CrabLeg {
  let b = bodies[leg.parent];
  let cp = b.com + b.vel * 0.28s;
  if distance(leg.foot, cp) > 0.5m { leg.foot = Pt[World](cp.x, 0.0, cp.z); }
}
```

A law runs once per row of the kind it writes, and only it writes those fields.

## Constraint

```
constraint upright(b: CrabBody) -> Verdict {
  if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
  let margin = (b.pose * b.head).y - 0.15m;
  if margin < 0.0m { return Verdict::Missed(-margin); }
  return Verdict::Held(margin);
}
```

## Schedule

```
schedule Main {
  pass rows: crabs();
  pass motion: run_laws(body_law -> crab_step);
  pass check: check(upright);
  pass camera: orbit(at: Pt[World](0.0, 0.4, 0.0), radius: 6m, fov: 40deg);
  pass cull: build_indirect(Ground, CrabLeg, Claw);
  pass vis: render_bundle(Vis64) { draw_indirect(Ground, CrabLeg, Claw); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```

## Raw WGSL

```
fn hash(p: vec3<f32>) -> f32 {
  raw wgsl (p: vec3<f32>) -> f32 { return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5); }
}
```

Raw code is opaque: not differentiable, bound `Unbounded`.
