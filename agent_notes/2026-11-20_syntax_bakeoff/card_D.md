# Loft language card

Loft has a Python-like surface that compiles to GPU code. An `@kind` class is code and compiles once; its annotated
fields are a row, one instance's data. `@comptime` functions run at build and emit rows. No Python runtime: no lists,
dicts, recursion or closures in GPU code; `float` is f32, `int` is i32.

## Spaces and units

```python
space("World", "Body", "Leg", "Claw", "Face", "Shell")
p = Pt["Claw"](0.0, 0.2, 0.9)          # components are metres
w: Pt["World"] = self.pose @ p         # pose: Xform["Claw", "World"]
```

Pt - Pt is a Vec; Pt + Pt is an error; mixing spaces is E104. `Dir[S]` and `Norm[S]` have unit length; normals map by
inverse transpose. `inv(x)` inverts an Xform. Units `Meters Rad Sec Kg`; write `0.3*m 1.2*rad 0.5*s 40*deg`.

## Kind and row

```python
@kind
class CrabLeg:
    parent: RowId["CrabBody"]
    pose: Xform["Leg", "World"]
    femur: Meters
    foot: Pt["World"]

    @query
    def trace(self, p: Pt["Leg"]) -> Field["Leg"]:
        return sd_capsule(p, Pt["Leg"](0, 0, 0), Pt["Leg"](self.femur, 0, 0), 0.07*m)

@kind
class Claw:
    serrated: Static[bool]      # a code variant, not a row field; discrete only
```

`Static[Meters]` is E401; over 4 used variants is E402.

## Comptime: emitting rows

```python
@comptime
def crab(b: RowId["CrabBody"], body: Xform["Body", "World"]):
    for j in range(6):
        for side in (-1.0, 1.0):
            emit(CrabLeg(parent=b, pose=body @ leg_socket(j, side), femur=0.40*m + 0.05*m*j,
                         foot=body @ Pt["Body"](side, 0.0, 0.3*j)))
```

`emit` appends a row and returns its `RowId`, never a shader. A loop inside a kind method is a GPU loop.

## Queries

A kind implements any `@query` method named: `trace(self, p) -> Field[S]` (sphere-traced distance);
`surface(self, u, v) -> Pt[S]` with `@query(grid=(nu, nv))` (rasterised patch); `visible(self, ro, rd) -> float`
(closed-form hit t, -1 on miss); `material`, `light`, `sky`, `medium`. Any query may take `time: Sec`.

The compiler derives every `Field` bound. A divisor that can reach zero is E211: clamp it.
`diff(f, "u")` needs C1; `diff` of `min`, `abs` or Voronoi is E307.

```python
@scan(over=(0.0, 1.0), steps=256, schedule="bake_lut")
class whorl:
    def init(u: float) -> Frame["Shell"]:
        return frame_from_tangent(spine(u), normalize(diff(spine, "u")(u)), Dir["Shell"](0, 1, 0))
    def step(prev: Frame["Shell"], u: float) -> Frame["Shell"]:
        return transport_frame(prev, spine(u), normalize(diff(spine, "u")(u)))
# f = sample_c2(whorl, u);  f.origin, f.normal, f.binormal
```

## Domain maps

```python
sym = mirror_x(q) | translate(-Vec["Face"](0.15, 0.72, 0.42))
eye_c = sym.pt(0.03, 0.30, 0.07)                 # Pt[sym]
globe = sd_sphere(sym.p - eye_c, 0.085*m)
eye_w = self.pose @ sym.to_parent(eye_c)
look = sym.from_parent(inv(self.pose) @ normalize(self.gaze - eye_w))
```

A map makes a fresh space `sym`; `sym.cell` is the branch. Also `rot_x(a)`, `rot_z(a)`, `repeat_polar(7)`.

## Materials

```python
@query
def material(self, p: Pt["Leg"], n: Norm["Leg"]) -> Bsdf:
    return ggx(albedo, 0.35, 0.0) + sss(vec3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07)

@lobe
def ink(dark: vec3, cut: float, n: vec3, v: vec3, l: vec3) -> vec4:   # a: passed below
    return vec4(dark if dot(n, l) < cut else vec3(0.92, 0.85, 0.7), 0.0)
```

A material sums lobes, bottom first: `ggx(albedo, rough, metal)`, `clearcoat(weight, rough)`,
`thin_film(weight, ior, nm)`, `sss(color, radius)`, `emission(color)`, or any `@lobe` function.
Library: `sd_sphere sd_box sd_round_box sd_capsule smin_poly ssub_poly fbm2 ray_capsule hit_min`.

## Motion law

```python
@law(writes=CrabLeg)
def crab_step(leg: CrabLeg, dt: Sec, bodies: Table["CrabBody"]):
    b = bodies[leg.parent]
    cp = b.com + b.vel * (0.28*s)
    if distance(leg.foot, cp) > 0.5*m:
        leg.foot = Pt["World"](cp.x, 0.0, cp.z)
```

A law runs once per row of the kind it writes, and only it writes those fields.

## Constraint

```python
@constraint
def upright(b: CrabBody) -> Verdict[Meters]:
    if not is_finite(b.pose):
        return Unknown("NonFinitePose")
    margin = (b.pose @ b.head).y - 0.15*m
    if margin < 0*m:
        return Missed(-margin)
    return Held(margin)
```

## Schedule

```python
@schedule
def main():                                # each call is a pass, in order
    rows(crabs)
    run_laws(body_law, crab_step)
    check(upright)
    orbit(at=Pt["World"](0.0, 0.4, 0.0), radius=6*m, fov=40*deg)
    build_indirect(Ground, CrabLeg, Claw)
    with render_bundle(Vis64):
        draw_indirect(Ground, CrabLeg, Claw)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=sun_sky)
    tonemap_aces(ColorHdr, sky=sky_color)
```

## Raw WGSL

```python
@raw_wgsl
def hash(p: vec3) -> float:
    """return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5);"""
```

Raw code is opaque: not differentiable, bound `Unbounded`.
