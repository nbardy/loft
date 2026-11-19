# Loft language card

Loft is WGSL plus modules (`import std::render::*;`), lambdas and declarations. `pub comptime fn scene() -> Scene`
runs at build.

## Spaces and units

```
space Body, Leg, Claw, Face, Shell;            // World is built in
let p = Pt[Claw](0, 0.2, 0.9);                 // components are metres
let q = xf * p;                                // xf: Xform[Claw, World]
```

Pt - Pt is a Vec; Pt + Pt, mixed spaces: E104. `Dir[S]` and `Norm[S]` have unit length; normals map by
inverse transpose; `inverse(xf)` inverts. Suffixes: `0.3m 1.2rad 0.5s`.

## Kinds and rows

`add` calls with the same geometry and material functions share one kind. A value a lambda captures (`femur`
below) is a row column; a literal in a function body is code. Law state is a declared kind,
`kind CrabLeg { row { parent: Row[CrabBody], foot: Pt[World], } }`; `emit` adds a row and returns a handle. Variants:
`kind K[comptime serrated: bool]`. `[comptime len: Meters]` is E401; over 4 used variants is E402.

## Comptime

```
pub comptime fn scene() -> Scene {
    let w = world();
    let crab = emit CrabBody { pose: rigid[Body, World](Vec[World](0, 0.4, 0)), ... };
    comptime for i in 0u..12u {
        let side = select(-1.0, 1.0, i % 2u == 1u);
        emit CrabLeg { parent: crab, foot: Pt[World](side, 0, 0.3 * f32(i / 2u)) };
        let at = attachment(crab, |r| r.pose * leg_socket(i / 2u, side));
        let femur = 0.40m + 0.05m * f32(i / 2u);
        add(w, field(box(Pt[Leg](-0.1, -0.1, -0.1), Pt[Leg](0.8, 0.1, 0.1)),
                     |p| sd_capsule(p, Pt[Leg](0, 0, 0), Pt[Leg](femur, 0, 0), 0.07m)), wet, at);
    }
    add_sun(w, normalize(Vec[World](-3, 5, 2)), rgb(3, 3, 2.5));
    environment(w, |d| rgb(0.4, 0.5, 0.6));
    return frame(w, look_at(eye, target, up, 0.6rad), main);
}
```

## Geometry and queries

`field(box, |p| dist)` is sphere-traced; `surface(|uv| point, grid(nu, nv))` is rasterised; `capsule(a, b, r)`,
`sphere(r)`, `plane(n, offset)` are closed form. `time()` is the clock. Bounds
are derived. A divisor reaching zero is E211. `diff(f, u)` needs C1; `diff` of `min`, `abs`, Voronoi: E307.

```
scan whorl(u: Unit) @schedule(bake_lut, 256u) -> Table[Basis[Shell]] {
    init: frame_seed(spine(0), normalize(diff(spine, u)(0)), Dir[Shell](0, 1, 0));
    step(last, u_i): transport_step(last, spine(u_i), normalize(diff(spine, u)(u_i)));
}
// sample_c2(whorl, u) -> .origin .normal .binormal
```

## Domain maps

```
map e = mirror_x(p) |> translate(Vec(-0.15, -0.72, -0.42));
let c = Pt['e](0.03, 0.30, 0.07);
let globe = length(e.p - c) - 0.085m;
let eye_w = xf * (e.to_parent * c);
let look = e.from_parent.dir(inverse(xf) * normalize(target - eye_w));
```

`map` makes a fresh space `'e`; `e.cell` is the branch. Also `rotate(axis, a)`, `repeat_polar(p, 7u)`.

## Materials

```
fn wet(h: Hit) -> Bsdf {
    return coat(sss(ggx(rgb(0.6, 0.14, 0.05), 0.35, 0.0), rgb(0.9, 0.3, 0.1), 0.02m), 0.7, 0.07, 1.34);
}
fn ink(w: World, h: Hit, wo: DirW, k: Key) -> Rgb {
    let src = light(w, h.p, k);
    return select(rgb(0.92, 0.85, 0.7), rgb(0.1, 0.1, 0.12), dot(h.n, src.wi) < 0.4);
}
```

Materials are `fn(Hit) -> Bsdf`; layers wrap: `ggx(base, rough, metal)`, `coat(b, weight, rough, ior)`,
`thin_film(b, ior, thickness)`, `sss(b, color, radius)`, `emission(b, color)`. Looks (`ink`) go to `deferred`. Also: `sd_box sd_round_box sd_capsule smin_poly ssub_poly fbm2`.

## Motion law

```
pub law crab_step(leg: CrabLeg, bodies: Table[CrabBody], dt: Sec) -> CrabLeg {
    let b = bodies[leg.parent];
    var next = leg;
    let cp = b.com + b.vel * 0.28s;
    if distance(leg.foot, cp) > 0.5m { next.foot = Pt[World](cp.x, 0, cp.z); }
    return next;
}
```

Each tick a law returns its row's replacement; only it writes that row.

## Constraint

```
pub constraint upright(b: CrabBody) -> Verdict[Meters] {
    if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
    let margin = (b.pose * b.head).y - 0.15m;
    if margin < 0m { return Verdict::Missed(-margin); }
    return Verdict::Held(margin);
}
```

## Schedule

```
pub fn main(w: World, c: Camera) -> Schedule {
    let s = plan(w, c);                         // calls are passes, in order
    run_laws(s, w, [body_law, crab_step]);
    check(s, w, upright);
    let n = clear_counts(s, w);
    let v = record(s, w, c, cull(s, w, c, n), vis64());
    let hdr = deferred(s, w, v, classify(s, v, n), sun_and_sky);
    return output(s, hdr, aces);
}
```

## Raw WGSL

```
fn hash(p: vec3f) -> f32 {
    return raw wgsl(p: vec3f) -> f32 { return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5); };
}
```

Raw code is opaque: not differentiable, bound `Unbounded`.
