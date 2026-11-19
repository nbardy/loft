# Task 1: hermit crab

```python
space("World", "Body", "Leg", "Claw", "Face", "Shell")

TAU = 6.2831853*rad

def num(x: Meters) -> float:
    return x / (1.0*m)

def wet_chitin(noise: float) -> Bsdf:
    rough = 0.28 + 0.22*noise
    return ggx(vec3(0.85, 0.22, 0.07), rough, 0.0) + sss(vec3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07)

# ---------------------------------------------------------------- shell

def spine(u: float) -> Pt["Shell"]:
    a = u * 3.0 * TAU
    r = 0.006 + 0.05*u
    return Pt["Shell"](r*cos(a), 0.16*u, r*sin(a))

@scan(over=(0.0, 1.0), steps=256, schedule="bake_lut")
class whorl:
    def init(u: float) -> Frame["Shell"]:
        return frame_from_tangent(spine(u), normalize(diff(spine, "u")(u)), Dir["Shell"](0, 1, 0))
    def step(prev: Frame["Shell"], u: float) -> Frame["Shell"]:
        return transport_frame(prev, spine(u), normalize(diff(spine, "u")(u)))

@kind
class Shell:
    pose: Xform["Shell", "World"]

    @query(grid=(512, 48))
    def surface(self, u: float, v: float) -> Pt["Shell"]:
        f = sample_c2(whorl, u)
        th = v * TAU
        rib = 1.0 + 0.07*sin(8.0*th) + 0.03*sin(u * 160.0*rad)
        r = (0.004 + 0.034*u) * rib
        return f.origin + (f.normal*cos(th) + f.binormal*sin(th)) * r

    @query
    def material(self, p: Pt["Shell"], n: Norm["Shell"]) -> Bsdf:
        band = 0.5 + 0.5*sin(num(p.y) * 300.0*rad)
        return ggx(mix(vec3(0.86, 0.78, 0.64), vec3(0.45, 0.28, 0.18), band), 0.45, 0.0) + clearcoat(0.5, 0.12)

# ---------------------------------------------------------------- body and legs

@kind
class CrabBody:
    pose: Xform["Body", "World"]
    vel: Vec["World"]

    @query
    def trace(self, p: Pt["Body"]) -> Field["Body"]:
        return sd_round_box(p - Pt["Body"](0.0, 0.0, 0.0), Vec["Body"](0.09, 0.04, 0.12), 0.03*m)

    @query
    def material(self, p: Pt["Body"], n: Norm["Body"]) -> Bsdf:
        return wet_chitin(fbm2(vec2(num(p.x), num(p.z)) * 120.0))

@kind
class CrabLeg:
    parent: RowId["CrabBody"]
    pose: Xform["Leg", "World"]
    femur: Meters
    tibia: Meters
    foot: Pt["World"]
    home: Pt["Body"]

    @query
    def trace(self, p: Pt["Leg"]) -> Field["Leg"]:
        foot = inv(self.pose) @ self.foot
        knee = Pt["Leg"](self.femur*0.8, self.femur*0.55, 0.0)
        d = foot - knee
        tip = knee + d * (self.tibia / max(length(d), 0.001*m))
        thigh = sd_capsule(p, Pt["Leg"](0, 0, 0), knee, 0.016*m)
        shin = sd_capsule(p, knee, tip, 0.011*m)
        return smin_poly(thigh, shin, 0.01*m)

    @query
    def material(self, p: Pt["Leg"], n: Norm["Leg"]) -> Bsdf:
        return wet_chitin(fbm2(vec2(num(p.x), num(p.y)) * 200.0))

@law(writes=CrabLeg)
def crab_step(leg: CrabLeg, dt: Sec, bodies: Table["CrabBody"]):
    b = bodies[leg.parent]
    home = b.pose @ leg.home
    target = home + b.vel * (0.28*s)
    if distance(leg.foot, target) > 0.04*m:
        leg.foot = Pt["World"](target.x, 0.0, target.z)

# ---------------------------------------------------------------- claws (one code, two rows)

@kind
class Claw:
    serrated: Static[bool]
    pose: Xform["Claw", "World"]
    size: Meters
    open: Rad

    @query
    def trace(self, p: Pt["Claw"]) -> Field["Claw"]:
        s = self.size
        palm = sd_round_box(p - Pt["Claw"](0.0, 0.0, 0.0), Vec["Claw"](0.5*s, 0.4*s, 0.35*s), 0.12*s)
        fixed = sd_capsule(p, Pt["Claw"](0.4*s, -0.12*s, 0.0), Pt["Claw"](1.3*s, -0.12*s, 0.0), 0.1*s)
        hinge = translate(p, -Vec["Claw"](0.45*s, 0.12*s, 0.0)) | rot_z(-self.open)
        mv = sd_capsule(hinge.p, hinge.pt(0.0, 0.0, 0.0), hinge.pt(0.9*s, 0.0, 0.0), 0.1*s)
        if self.serrated:
            pitch = max(0.14*num(s), 0.001)
            tooth = abs(fract(num(hinge.p.x) / pitch) - 0.5)
            mv = mv + 0.1*s*tooth
        return smin_poly(smin_poly(palm, fixed, 0.06*s), mv, 0.04*s)

    @query
    def material(self, p: Pt["Claw"], n: Norm["Claw"]) -> Bsdf:
        return wet_chitin(fbm2(vec2(num(p.x), num(p.z)) * 160.0))

# ---------------------------------------------------------------- eyes (one mirrored definition)

def gaze_target(time: Sec) -> Pt["World"]:
    a = 0.4*rad*(time/s)
    return Pt["World"](0.25*sin(a), 0.10 + 0.04*cos(1.7*a), 0.40 + 0.1*cos(a))

def stalk_d(p: Pt["Face"]) -> Field["Face"]:
    sym = mirror_x(p) | translate(-Vec["Face"](0.03, 0.0, 0.0))
    return sd_capsule(sym.p, sym.pt(0.0, 0.0, 0.0), sym.pt(0.0, 0.04, 0.0), 0.006*m)

def globe_d(p: Pt["Face"]) -> Field["Face"]:
    sym = mirror_x(p) | translate(-Vec["Face"](0.03, 0.0, 0.0))
    eye_c = sym.pt(0.0, 0.04, 0.0)
    return sd_sphere(sym.p - eye_c, 0.012*m)

def pupil_d(pose: Xform["Face", "World"], p: Pt["Face"], time: Sec) -> Field["Face"]:
    sym = mirror_x(p) | translate(-Vec["Face"](0.03, 0.0, 0.0))
    eye_c = sym.pt(0.0, 0.04, 0.0)
    eye_w = pose @ sym.to_parent(eye_c)
    look = sym.from_parent(inv(pose) @ normalize(gaze_target(time) - eye_w))
    return sd_sphere(sym.p - (eye_c + look*(0.009*m)), 0.0045*m)

@kind
class CrabEyes:
    pose: Xform["Face", "World"]

    @query
    def trace(self, p: Pt["Face"], time: Sec) -> Field["Face"]:
        return min(min(stalk_d(p), globe_d(p)), pupil_d(self.pose, p, time))

    @query
    def material(self, p: Pt["Face"], n: Norm["Face"], time: Sec) -> Bsdf:
        if pupil_d(self.pose, p, time) < 0.0004*m:
            return ggx(vec3(0.02, 0.02, 0.02), 0.1, 0.0) + clearcoat(0.9, 0.03)
        if globe_d(p) < 0.0004*m:
            return ggx(vec3(0.9, 0.85, 0.7), 0.2, 0.0) + clearcoat(0.9, 0.03)
        return wet_chitin(fbm2(vec2(num(p.x), num(p.y)) * 300.0))

# ---------------------------------------------------------------- world

@kind
class Ground:
    level: Meters

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return sd_box(p - Pt["World"](0.0, self.level - 0.1*m, 0.0), Vec["World"](4.0*m, 0.1*m, 4.0*m))

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        grain = fbm2(vec2(num(p.x), num(p.z)) * 60.0)
        return ggx(vec3(0.30, 0.24, 0.17) * (0.8 + 0.4*grain), 0.55, 0.0) + clearcoat(0.4, 0.15)

@kind
class SunSky:
    dir: Dir["World"]
    color: vec3
    zenith: vec3
    horizon: vec3

    @query
    def light(self, p: Pt["World"]) -> Light:
        return directional(self.dir, self.color)

    @query
    def sky(self, rd: Dir["World"]) -> vec3:
        return mix(self.horizon, self.zenith, clamp(rd.y, 0.0, 1.0))

sun_sky = SunSky
sky_color = SunSky

# ---------------------------------------------------------------- build: rows

@comptime
def leg_socket(j: int, side: float) -> Xform["Leg", "Body"]:
    yaw = (0.1*(j - 2.5) + (0.0 if side > 0.0 else 3.1416))*rad
    return Xform["Leg", "Body"](t=Vec["Body"](side*0.07, -0.01, 0.035*j - 0.08), yaw=yaw)

@comptime
def crab(b: RowId["CrabBody"], body: Xform["Body", "World"]):
    for j in range(6):
        for side in (-1.0, 1.0):
            emit(CrabLeg(parent=b, pose=body @ leg_socket(j, side),
                         femur=0.07*m + 0.008*m*j, tibia=0.09*m + 0.01*m*j,
                         foot=body @ Pt["Body"](side*0.14, -0.05, 0.03*j - 0.07),
                         home=Pt["Body"](side*0.14, -0.05, 0.03*j - 0.07)))
    emit(Shell(pose=body @ Xform["Shell", "Body"](t=Vec["Body"](0.0, 0.07, -0.06), yaw=0.3*rad)))
    emit(Claw(serrated=True, pose=body @ Xform["Claw", "Body"](t=Vec["Body"](0.11, 0.02, 0.15), yaw=-1.2*rad),
              size=0.05*m, open=0.45*rad))
    emit(Claw(serrated=True, pose=body @ Xform["Claw", "Body"](t=Vec["Body"](-0.11, 0.02, 0.15), yaw=-1.9*rad),
              size=0.028*m, open=0.2*rad))
    emit(CrabEyes(pose=body @ Xform["Face", "Body"](t=Vec["Body"](0.0, 0.04, 0.12), yaw=0.0*rad)))

@comptime
def crabs():
    body = Xform["Body", "World"](t=Vec["World"](0.0, 0.05, 0.0), yaw=0.0*rad)
    b = emit(CrabBody(pose=body, vel=Vec["World"](0.0, 0.0, 0.0)))
    crab(b, body)

@comptime
def lights():
    emit(Ground(level=0.0*m))
    emit(SunSky(dir=normalize(Vec["World"](-0.4, -0.8, -0.3)), color=vec3(3.0, 2.8, 2.5),
                zenith=vec3(0.35, 0.55, 0.9), horizon=vec3(0.8, 0.85, 0.9)))

@schedule
def main():
    rows(crabs)
    rows(lights)
    run_laws(crab_step)
    orbit(at=Pt["World"](0.0, 0.08, 0.0), radius=0.7*m, fov=40*deg)
    build_indirect(Ground, CrabBody, CrabLeg, Claw, CrabEyes, Shell)
    with render_bundle(Vis64):
        draw_indirect(Ground, CrabBody, CrabLeg, Claw, CrabEyes, Shell)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=sun_sky)
    tonemap_aces(ColorHdr, sky=sky_color)
```

# Task 2: reed bed in wind

```python
space("World", "Reed")

TAU = 6.2831853*rad
PAPER = vec3(0.92, 0.85, 0.70)
INK = vec3(0.14, 0.12, 0.20)

def num(x: Meters) -> float:
    return x / (1.0*m)

@lobe
def flat(c: vec3, n: vec3, v: vec3, l: vec3) -> vec4:
    return vec4(c, 0.0)

@lobe
def ink(dark: vec3, cut: float, n: vec3, v: vec3, l: vec3) -> vec4:
    return vec4(dark if dot(n, l) < cut else PAPER, 0.0)

@kind
class Reed:
    pose: Xform["Reed", "World"]
    height: Meters
    lean: Rad
    stiff: float
    phase: Rad

    @query
    def trace(self, p: Pt["World"], time: Sec) -> Field["World"]:
        q = inv(self.pose) @ p
        h = max(self.height, 0.2*m)
        t = clamp(q.y / h, 0.0, 1.0)
        base = self.pose @ Pt["Reed"](0.0, 0.0, 0.0)
        ph = self.phase + (2.2*num(base.x) + 1.3*num(base.z) - 1.6*(time/s))*rad
        gust = 0.6 + 0.4*sin(ph)
        bend = 0.30*m * (t*t) * gust / (0.4 + max(self.stiff, 0.0))
        wind = normalize(inv(self.pose) @ Vec["World"](1.0, 0.0, 0.3))
        shift = Vec["Reed"](sin(self.lean)*q.y, 0.0, 0.0) + wind*bend
        r = q - shift
        radius = 0.0055*m * (1.0 - 0.45*t)
        stalk = sd_capsule(r, Pt["Reed"](0.0, 0.0, 0.0), Pt["Reed"](0.0, h, 0.0), radius)
        head = sd_capsule(r, Pt["Reed"](0.0, 0.86*h, 0.0), Pt["Reed"](0.0, h + 0.03*m, 0.0), 0.014*m)
        return min(stalk, head)

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ink(INK, 0.12)

@kind
class Water:
    level: Meters

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return sd_box(p - Pt["World"](0.0, self.level - 0.1*m, 0.0), Vec["World"](8.0*m, 0.1*m, 8.0*m))

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        band = step(0.92, fract(num(p.z)*14.0 + 0.3*sin(num(p.x)*5.0*rad)))
        return flat(mix(PAPER, vec3(0.62, 0.72, 0.76), 0.35*band))

@kind
class SunSky:
    dir: Dir["World"]
    color: vec3

    @query
    def light(self, p: Pt["World"]) -> Light:
        return directional(self.dir, self.color)

    @query
    def sky(self, rd: Dir["World"]) -> vec3:
        return mix(PAPER, vec3(0.80, 0.84, 0.84), 0.5*clamp(rd.y, 0.0, 1.0))

sun_sky = SunSky
sky_color = SunSky

@comptime
def rnd(i: int, k: float) -> float:
    return fract(sin(i*12.9898 + k*78.233) * 43758.5453)

@comptime
def bed():
    for i in range(200):
        a = (i * 2.39996)*rad
        blob = 1.0 + 0.35*sin(3.0*a + rnd(i, 9.0)*TAU)
        r = 0.09 * sqrt(i + 0.5) * (0.75 + 0.5*rnd(i, 1.0)) * blob
        x = r*cos(a) + 0.15*(rnd(i, 2.0) - 0.5)
        z = r*sin(a) + 0.15*(rnd(i, 3.0) - 0.5)
        emit(Reed(pose=Xform["Reed", "World"](t=Vec["World"](x, 0.0, z), yaw=rnd(i, 4.0)*TAU),
                  height=(0.9 + 0.7*rnd(i, 5.0))*m,
                  lean=(rnd(i, 6.0) - 0.5)*0.30*rad,
                  stiff=0.4 + 0.6*rnd(i, 7.0),
                  phase=rnd(i, 8.0)*TAU))

@comptime
def scene():
    emit(Water(level=0.0*m))
    emit(SunSky(dir=normalize(Vec["World"](-0.6, -0.7, -0.2)), color=vec3(1.0, 1.0, 1.0)))

@schedule
def main():
    rows(bed)
    rows(scene)
    orbit(at=Pt["World"](0.0, 0.6, 0.0), radius=4.0*m, fov=40*deg)
    build_indirect(Water, Reed)
    with render_bundle(Vis64):
        draw_indirect(Water, Reed)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=sun_sky)
    tonemap_aces(ColorHdr, sky=sky_color)
```

# Task 3: heron's hop

```python
space("World", "Trunk")

TAU = 6.2831853*rad
NECK_LIMIT = 1.0*rad
HOP = 0.5*s
HOP_H = 0.22*m
STAND = 0.55*m

def wrap(a: Rad) -> Rad:
    return a - TAU*floor((a + 0.5*TAU)/TAU)

def trunk_pose(pos: Pt["World"], heading: Rad, y: Meters) -> Xform["Trunk", "World"]:
    return Xform["Trunk", "World"](t=Vec["World"](pos.x, pos.y + STAND + y, pos.z), yaw=heading)

def pivot_of(pose: Xform["Trunk", "World"]) -> Pt["World"]:
    return pose @ Pt["Trunk"](0.0, 0.40, 0.18)

# signed yaw from the trunk heading to the target, seen from the head pivot
def aim_yaw(pose: Xform["Trunk", "World"], heading: Rad, target: Pt["World"]) -> Rad:
    d = target - pivot_of(pose)
    return wrap(atan2(d.x, d.z) - heading)

def aim_pitch(pose: Xform["Trunk", "World"], target: Pt["World"]) -> Rad:
    d = target - pivot_of(pose)
    return atan2(d.y, length(Vec["World"](d.x, 0.0, d.z)))

def fish_curve(t: Sec) -> Pt["World"]:
    a = 0.35*rad*(t/s)
    r = 1.2 + 0.15*sin(2.0*a)
    return Pt["World"](r*sin(a), 0.0, r*cos(a))

# ---------------------------------------------------------------- kinds

@kind
class Fish:
    t: Sec
    pos: Pt["World"]

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return sd_capsule(p, self.pos - Vec["World"](0.0, 0.0, 0.04), self.pos + Vec["World"](0.0, 0.0, 0.04), 0.02*m)

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.7, 0.75, 0.8), 0.25, 0.8) + clearcoat(0.6, 0.08)

def leg_d(q: Pt["Trunk"], side: float, y: Meters) -> Field["Trunk"]:
    hip = Pt["Trunk"](side*0.045, -0.05, -0.05)
    ankle = Pt["Trunk"](side*0.045, -0.30 - 0.1*num_m(y), -0.08)
    foot = Pt["Trunk"](side*0.045, -STAND - 0.2*y, -0.03)
    return min(sd_capsule(q, hip, ankle, 0.012*m), sd_capsule(q, ankle, foot, 0.009*m))

def num_m(x: Meters) -> float:
    return x / (1.0*m)

@kind
class Trunk:
    pos: Pt["World"]
    heading: Rad
    y: Meters
    hop_t: Sec
    fish: RowId["Fish"]

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        q = inv(trunk_pose(self.pos, self.heading, self.y)) @ p
        body = sd_capsule(q, Pt["Trunk"](0.0, 0.0, -0.14), Pt["Trunk"](0.0, 0.02, 0.12), 0.085*m)
        legs = min(leg_d(q, 1.0, self.y), leg_d(q, -1.0, self.y))
        return smin_poly(body, legs, 0.02*m)

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.55, 0.58, 0.62), 0.5, 0.0)

def beak_d(pivot: Pt["World"], fwd: Dir["World"], p: Pt["World"]) -> Field["World"]:
    return sd_capsule(p, pivot + fwd*(0.08*m), pivot + fwd*(0.32*m), 0.012*m)

@kind
class Head:
    parent: RowId["Trunk"]
    fish: RowId["Fish"]
    yaw: Rad
    pitch: Rad
    over: Rad
    trunk: Xform["Trunk", "World"]
    fwd: Dir["World"]

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        n0 = self.trunk @ Pt["Trunk"](0.0, 0.10, 0.20)
        n1 = self.trunk @ Pt["Trunk"](0.0, 0.20, 0.34)
        n2 = self.trunk @ Pt["Trunk"](0.0, 0.30, 0.10)
        n3 = pivot_of(self.trunk)
        neck = min(sd_capsule(p, n0, n1, 0.02*m), min(sd_capsule(p, n1, n2, 0.018*m), sd_capsule(p, n2, n3, 0.016*m)))
        skull = sd_capsule(p, n3, n3 + self.fwd*(0.09*m), 0.028*m)
        return min(smin_poly(neck, skull, 0.02*m), beak_d(n3, self.fwd, p))

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        if beak_d(pivot_of(self.trunk), self.fwd, p) < 0.002*m:
            return ggx(vec3(0.85, 0.65, 0.15), 0.35, 0.0)
        return ggx(vec3(0.55, 0.58, 0.62), 0.5, 0.0)

@kind
class Water:
    level: Meters

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return sd_box(p - Pt["World"](0.0, self.level - 0.1*m, 0.0), Vec["World"](4.0*m, 0.1*m, 4.0*m))

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.1, 0.25, 0.3), 0.08, 0.0) + clearcoat(0.8, 0.03)

@kind
class SunSky:
    dir: Dir["World"]
    color: vec3
    zenith: vec3
    horizon: vec3

    @query
    def light(self, p: Pt["World"]) -> Light:
        return directional(self.dir, self.color)

    @query
    def sky(self, rd: Dir["World"]) -> vec3:
        return mix(self.horizon, self.zenith, clamp(rd.y, 0.0, 1.0))

sun_sky = SunSky
sky_color = SunSky

# ---------------------------------------------------------------- laws

@law(writes=Fish)
def fish_step(f: Fish, dt: Sec):
    f.t = f.t + dt
    f.pos = fish_curve(f.t)

# the body decides to hop from the same aim the neck uses, so both agree on "at the limit"
@law(writes=Trunk)
def body_step(t: Trunk, dt: Sec, fish: Table["Fish"]):
    f = fish[t.fish]
    want = aim_yaw(trunk_pose(t.pos, t.heading, t.y), t.heading, f.pos)
    if t.hop_t > 0.0*s:
        t.hop_t = t.hop_t + dt
        if t.hop_t >= HOP:
            t.hop_t = 0.0*s
            t.y = 0.0*m
        else:
            u = t.hop_t / HOP
            t.y = HOP_H * (4.0 * u * (1.0 - u))
            reach = (5.0*rad/s) * dt
            t.heading = t.heading + clamp(want, -reach, reach)
    elif abs(want) > NECK_LIMIT:
        t.hop_t = dt

@law(writes=Head)
def neck_step(h: Head, dt: Sec, trunks: Table["Trunk"], fish: Table["Fish"]):
    t = trunks[h.parent]
    f = fish[h.fish]
    pose = trunk_pose(t.pos, t.heading, t.y)
    aim = aim_yaw(pose, t.heading, f.pos)
    yaw = clamp(aim, -NECK_LIMIT, NECK_LIMIT)
    pitch = clamp(aim_pitch(pose, f.pos), -1.0*rad, 0.5*rad)
    yw = t.heading + yaw
    h.yaw = yaw
    h.pitch = pitch
    h.over = abs(aim) - abs(yaw)
    h.trunk = pose
    h.fwd = normalize(Vec["World"](sin(yw)*cos(pitch), sin(pitch), cos(yw)*cos(pitch)))

# ---------------------------------------------------------------- constraints

@constraint
def gaze_held(h: Head) -> Verdict[Rad]:
    if not is_finite(h.yaw):
        return Unknown("NonFiniteYaw")
    if h.over > 0.0*rad:
        return Missed(h.over)
    return Held(NECK_LIMIT - abs(h.yaw))

@constraint
def head_above_hips(h: Head) -> Verdict[Meters]:
    if not is_finite(h.trunk):
        return Unknown("NonFinitePose")
    hips = h.trunk @ Pt["Trunk"](0.0, -0.05, -0.08)
    head = pivot_of(h.trunk) + h.fwd*(0.05*m)
    margin = head.y - hips.y
    if margin < 0*m:
        return Missed(-margin)
    return Held(margin)

# ---------------------------------------------------------------- build and schedule

@comptime
def scene():
    f = emit(Fish(t=0.0*s, pos=fish_curve(0.0*s)))
    t = emit(Trunk(pos=Pt["World"](0.0, 0.0, 0.0), heading=0.0*rad, y=0.0*m, hop_t=0.0*s, fish=f))
    emit(Head(parent=t, fish=f, yaw=0.0*rad, pitch=0.0*rad, over=0.0*rad,
              trunk=trunk_pose(Pt["World"](0.0, 0.0, 0.0), 0.0*rad, 0.0*m),
              fwd=Dir["World"](0.0, 0.0, 1.0)))
    emit(Water(level=0.0*m))
    emit(SunSky(dir=normalize(Vec["World"](-0.4, -0.8, -0.3)), color=vec3(3.0, 2.8, 2.5),
                zenith=vec3(0.35, 0.55, 0.9), horizon=vec3(0.8, 0.85, 0.9)))

@schedule
def main():
    rows(scene)
    run_laws(fish_step)
    run_laws(body_step)
    run_laws(neck_step)
    check(gaze_held)
    check(head_above_hips)
    orbit(at=Pt["World"](0.0, 0.6, 0.5), radius=3.5*m, fov=40*deg)
    build_indirect(Water, Trunk, Head, Fish)
    with render_bundle(Vis64):
        draw_indirect(Water, Trunk, Head, Fish)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=sun_sky)
    tonemap_aces(ColorHdr, sky=sky_color)
```
