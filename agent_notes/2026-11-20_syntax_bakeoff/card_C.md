# Loft language card

Loft is a Slang/HLSL-style shader language. A `[Kind]` struct is code and compiles once; its fields are a row, one
instance's data. `[Comptime]` functions run at build and emit rows.

## Spaces and units

```hlsl
struct Claw : ISpace {};                       // likewise World, Body, Leg, Face, Shell
Pt<Claw> p = Pt<Claw>(0.0, 0.2, 0.9);          // components are metres
Pt<World> w = pose * p;                        // Xform<Claw, World> pose
```

Pt - Pt is a Vec; Pt + Pt is an error; mixing spaces is E104. `Dir<S>` and `Norm<S>` have unit length; normals map by
inverse transpose. `inverse(x)` inverts an Xform. Units: `Meters Rad Sec Kg`; suffixes `0.3m 1.2rad 0.5s 40deg`.

## Kind and row

```hlsl
[Kind]
struct CrabLeg : ITrace<Leg>
{
    RowId<CrabBody> parent;
    Xform<Leg, World> pose;
    Meters femur;
    Pt<World> foot;

    Field<Leg> trace(Pt<Leg> p) { return sd_capsule(p, Pt<Leg>(0, 0, 0), Pt<Leg>(femur, 0, 0), 0.07m); }
};
[Kind] struct Claw<let Serrated : bool> : ITrace<Claw> { ... };   // discrete only
```

`struct K<let Len : Meters>` is E401; over 4 used variants is E402.

## Comptime: emitting rows

```hlsl
[Comptime]
void crab(RowId<CrabBody> b, Xform<Body, World> body)
{
    for (uint j = 0; j < 6; j++)
        for (float side = -1.0; side <= 1.0; side += 2.0)
            emit(CrabLeg{ .parent = b, .pose = body * leg_socket(j, side), .femur = 0.40m + 0.05m * j,
                          .foot = body * Pt<Body>(side, 0.0, 0.3 * j) });
}
```

`emit` appends a row and returns its `RowId`, never a shader. Loops in a kind method are GPU loops.

## Queries

A kind implements any of: `ITrace<S>` (`Field<S> trace(Pt<S> p)`, sphere-traced distance); `ISurface<S>`
(`[Grid(nu, nv)] Pt<S> surface(float u, float v)`, rasterised patch); `IVisible<S>` (`float visible(Pt<S> ro, Dir<S>
rd)`, closed-form hit t, -1 on miss); `IMaterial<S>`, `ILight`, `ISky`, `IMedium`. Any method may take `Sec time`.

The compiler derives every `Field` bound. A divisor that can reach zero is E211: clamp it.
`diff(f, u)` needs C1; `diff` of `min`, `abs` or Voronoi is E307.

```hlsl
[Scan(steps = 256, range = (0.0, 1.0), BakeLut)]
struct Whorl : IScan<Frame<Shell>>
{
    Frame<Shell> init(float u) { return frame_from_tangent(spine(u), normalize(diff(spine, u)(u)), Dir<Shell>(0, 1, 0)); }
    Frame<Shell> step(Frame<Shell> prev, float u) { return transport_frame(prev, spine(u), normalize(diff(spine, u)(u))); }
};
// Frame<Shell> f = sample_c2<Whorl>(u);  f.origin, f.normal, f.binormal
```

## Domain maps

```hlsl
let sym = map(q).mirror_x().translate(-Vec<Face>(0.15, 0.72, 0.42));
let eye_c = sym.pt(0.03, 0.30, 0.07);                     // Pt<sym.Space>
float globe = sd_sphere(sym.p - eye_c, 0.085m);
Pt<World> eye_w = pose * sym.to_parent(eye_c);
let look = sym.from_parent(inverse(pose) * normalize(gaze - eye_w));
```

`map` makes a fresh space `sym.Space`; `sym.cell` is the branch. Also `.rot_x(a)`, `.rot_z(a)`,
`.repeat_polar(7)`.

## Materials

```hlsl
Bsdf material(Pt<Leg> p, Norm<Leg> n)
{
    return ggx(albedo, 0.35, 0.0) + sss(float3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07);
}
[Lobe] float4 ink(float3 dark, float cut, float3 n, float3 v, float3 l)   // a: passed below
{
    return float4(dot(n, l) < cut ? dark : float3(0.92, 0.85, 0.7), 0.0);
}
```

A material sums lobes, bottom first: `ggx(albedo, rough, metal)`, `clearcoat(weight, rough)`,
`thin_film(weight, ior, nm)`, `sss(color, radius)`, `emission(color)`, or any `[Lobe]` function.
Library: `sd_sphere sd_box sd_round_box sd_capsule smin_poly ssub_poly fbm2 ray_capsule hit_min`.

## Motion law

```hlsl
[Law(Writes = CrabLeg)]
void crab_step(inout CrabLeg leg, Sec dt, Table<CrabBody> bodies)
{
    CrabBody b = bodies[leg.parent];
    Pt<World> cp = b.com + b.vel * 0.28s;
    if (distance(leg.foot, cp) > 0.5m) leg.foot = Pt<World>(cp.x, 0.0, cp.z);
}
```

A law runs once per row of the kind it writes, and only it writes those fields.

## Constraint

```hlsl
[Constraint]
Verdict<Meters> upright(CrabBody b)
{
    if (!is_finite(b.pose)) return Verdict::Unknown(NonFinitePose);
    Meters margin = (b.pose * b.head).y - 0.15m;
    if (margin < 0.0m) return Verdict::Missed(-margin);
    return Verdict::Held(margin);
}
```

## Schedule

```hlsl
technique Main
{
    pass Rows     { crabs(); }
    pass Motion   { run_laws(body_law, crab_step); }
    pass Check    { check(upright); }
    pass Camera   { orbit(Pt<World>(0.0, 0.4, 0.0), 6m, 40deg); }
    pass Cull     { build_indirect<Ground, CrabLeg, Claw>(); }
    pass Vis      { render_bundle(Vis64) { draw_indirect<Ground, CrabLeg, Claw>(); } }
    pass Classify { tile_bin_kinds(Vis64, 8); }
    pass Shade    { dispatch_material_tiles(Vis64, sun_sky); }
    pass Present  { tonemap_aces(ColorHdr, sky_color); }
}
```

## Raw WGSL

```hlsl
float hash(float3 p)
{
    __wgsl(p) { return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5); }
}
```

Raw code is opaque: not differentiable, bound `Unbounded`.
