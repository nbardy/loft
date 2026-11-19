# Task 1: a hermit crab

```python
space("World", "Body", "Leg", "Claw", "Face", "Shell", "Ground")

TAU = 6.2831853
PI = 3.14159265

# Shell spine: a spiral in metres, 2.5 turns, radius grows with u
def spine(u: float) -> Pt["Shell"]:
    a = TAU*2.5*u
    r = 0.05 + 0.15*u
    return Pt["Shell"](r*cos(a), r*sin(a), 0.3*u)

# Tube radius grows along the spiral
def tube_radius(u: float) -> float:
    return 0.03 + 0.07*u

@scan(over=(0.0, 1.0), steps=256, schedule="bake_lut")
class whorl:
    def init(u: float) -> Frame["Shell"]:
        return frame_from_tangent(spine(u), normalize(diff(spine, "u")(u)), Dir["Shell"](0, 1, 0))
    def step(prev: Frame["Shell"], u: float) -> Frame["Shell"]:
        return transport_frame(prev, spine(u), normalize(diff(spine, "u")(u)))

@kind
class Shell:
    parent: RowId["CrabBody"]
    pose: Xform["Shell", "World"]
    ribs: float

    @query(grid=(96, 24))
    def surface(self, u: float, v: float) -> Pt["Shell"]:
        f = sample_c2(whorl, u)
        t = TAU*v
        ribbing = 1.0 + 0.08*cos(TAU*self.ribs*u)
        r = tube_radius(u)*ribbing
        return f.origin + r*(cos(t)*f.normal + sin(t)*f.binormal)

@kind
class Claw:
    parent: RowId["CrabBody"]
    pose: Xform["Claw", "World"]
    opening: Rad          # per-instance hinge angle
    scale: float          # feeder claw is the same definition at half scale

    @query
    def trace(self, p: Pt["Claw"]) -> Field["Claw"]:
        s = self.scale
        palm = sd_round_box(p - Pt["Claw"](0.0, 0.0, 0.0), Vec["Claw"](0.10*s, 0.08*s, 0.06*s), 0.02*m*s)
        fixed = sd_capsule(p, Pt["Claw"](0.08*s, 0.0, 0.0), Pt["Claw"](0.20*s, 0.02*s, 0.0), 0.012*m*s)
        hinge = Pt["Claw"](0.08*s, -0.03*s, 0.0)
        moving = rot_z(self.opening)(p - hinge)
        teeth = 0.002*m*s*abs(sin(60.0*moving.x))
        finger = sd_round_box(moving, Vec["Claw"](0.14*s, 0.012*s, 0.02*s), 0.008*m*s) - teeth
        return smin_poly(smin_poly(palm, fixed, 0.02*m*s), finger, 0.02*m*s)

@kind
class CrabBody:
    pose: Xform["Body", "World"]
    com: Pt["World"]
    vel: Vec["World"]
    head: Pt["Body"]

    @query
    def material(self, p: Pt["Body"], n: Norm["Body"]) -> Bsdf:
        rough = 0.25 + 0.15*fbm2(vec2(p.x, p.z)*900.0)
        base = vec3(0.78, 0.26, 0.10)
        return ggx(base, rough, 0.0) + sss(vec3(0.9, 0.4, 0.2), 0.02) + clearcoat(0.8, 0.04)

@kind
class CrabLeg:
    parent: RowId["CrabBody"]
    pose: Xform["Leg", "World"]
    femur: Meters
    tibia: Meters
    foot: Pt["World"]

    @query
    def trace(self, p: Pt["Leg"]) -> Field["Leg"]:
        a = Pt["Leg"](0.0, 0.0, 0.0)
        knee = Pt["Leg"](self.femur, 0.0, 0.0)
        ankle = Pt["Leg"](self.femur + self.tibia*0.5, -self.tibia*0.8, 0.0)
        d1 = sd_capsule(p, a, knee, 0.07*m)
        d2 = sd_capsule(p, knee, ankle, 0.05*m)
        return smin_poly(d1, d2, 0.03*m)

@kind
class Eye:
    parent: RowId["CrabBody"]
    pose: Xform["Face", "World"]
    gaze: Pt["World"]
    clock: Sec

    @query
    def trace(self, p: Pt["Face"]) -> Field["Face"]:
        sym = mirror_x(p) | translate(-Vec["Face"](0.15, 0.72, 0.42))
        eye_c = sym.pt(0.03, 0.30, 0.07)
        globe = sd_sphere(sym.p - eye_c, 0.085*m)
        stalk = sd_capsule(p, Pt["Face"](0.15, 0.55, 0.42), sym.to_parent(eye_c), 0.012*m)
        return smin_poly(globe, stalk, 0.02*m)

    @query
    def material(self, p: Pt["Face"], n: Norm["Face"]) -> Bsdf:
        sym = mirror_x(p) | translate(-Vec["Face"](0.15, 0.72, 0.42))
        eye_c = sym.pt(0.03, 0.30, 0.07)
        eye_w = self.pose @ sym.to_parent(eye_c)
        look = sym.from_parent(inv(self.pose) @ normalize(self.gaze - eye_w))
        pupil = dot(normalize(sym.p - eye_c), look)
        albedo = vec3(0.02, 0.02, 0.02) if pupil > 0.9 else vec3(0.85, 0.80, 0.70)
        return ggx(albedo, 0.25, 0.0) + clearcoat(0.7, 0.05)

@kind
class Ground:
    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return p.y

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        wet = 0.5 + 0.5*fbm2(vec2(p.x, p.z)*6.0)
        return ggx(vec3(0.42, 0.36, 0.28)*(1.0 - 0.2*wet), 0.15 + 0.2*(1.0 - wet), 0.0)

@kind
class Sun:
    dir: Dir["World"]

# Socket transforms: leg sockets, claw mounts, shell and face mounts on the body
def leg_socket(j: int, side: float) -> Xform["Leg", "Body"]:
    return rot_y(side*(0.5 + 0.15*j)*rad)

CLAW_MOUNT: Xform["Claw", "Body"] = translate(Vec["Body"](0.18, 0.10, 0.25))
FEEDER_MOUNT: Xform["Claw", "Body"] = translate(Vec["Body"](-0.18, 0.08, 0.25))
SHELL_MOUNT: Xform["Shell", "Body"] = translate(Vec["Body"](0.0, 0.25, -0.20))
FACE_MOUNT: Xform["Face", "Body"] = translate(Vec["Body"](0.0, 0.10, 0.35))

@comptime
def crab(body_pose: Xform["Body", "World"]):
    b = emit(CrabBody(pose=body_pose, com=body_pose @ Pt["Body"](0.0, 0.0, 0.0),
                      vel=Vec["World"](0.0, 0.0, 0.0), head=Pt["Body"](0.0, 0.25, 0.35)))
    for j in range(6):
        for side in (-1.0, 1.0):
            emit(CrabLeg(parent=b, pose=body_pose @ leg_socket(j, side),
                         femur=0.40*m + 0.05*m*j, tibia=0.35*m + 0.03*m*j,
                         foot=body_pose @ Pt["Body"](side*0.3, 0.0, 0.3*j)))
    emit(Claw(parent=b, pose=body_pose @ CLAW_MOUNT, opening=0.5*rad, scale=1.0))
    emit(Claw(parent=b, pose=body_pose @ FEEDER_MOUNT, opening=0.3*rad, scale=0.5))
    emit(Shell(parent=b, pose=body_pose @ SHELL_MOUNT, ribs=9.0))
    emit(Eye(parent=b, pose=body_pose @ FACE_MOUNT, gaze=Pt["World"](0.0, 0.3, 1.0), clock=0.0*s))
    emit(Ground())
    emit(Sun(dir=Dir["World"](0.4, 0.8, 0.3)))

crab(translate(Vec["World"](0.0, 0.0, 0.0)) @ rot_y(0.0*rad))

@law(writes=CrabBody)
def body_law(b: CrabBody, dt: Sec):
    b.com = b.com + b.vel*dt

@law(writes=Eye)
def eye_track(e: Eye, dt: Sec):
    e.clock = e.clock + dt
    t = e.clock/s
    e.gaze = Pt["World"](0.3*sin(0.2*t), 0.25, 0.9 + 0.1*cos(0.13*t))

@law(writes=CrabLeg)
def crab_step(leg: CrabLeg, dt: Sec, bodies: Table["CrabBody"]):
    b = bodies[leg.parent]
    cp = b.com + b.vel * (0.28*s)
    if distance(leg.foot, cp) > 0.5*m:
        leg.foot = Pt["World"](cp.x, 0.0, cp.z)

@constraint
def upright(b: CrabBody) -> Verdict[Meters]:
    if not is_finite(b.pose):
        return Unknown("NonFinitePose")
    margin = (b.pose @ b.head).y - 0.15*m
    if margin < 0*m:
        return Missed(-margin)
    return Held(margin)

@schedule
def main():
    rows(CrabBody, CrabLeg, Claw, Shell, Eye, Ground, Sun)
    run_laws(body_law, crab_step, eye_track)
    check(upright)
    orbit(at=Pt["World"](0.0, 0.1, 0.0), radius=1.5*m, fov=40*deg)
    build_indirect(Ground, CrabLeg, Claw, Shell, Eye, CrabBody)
    with render_bundle(Vis64):
        draw_indirect(Ground, CrabLeg, Claw, Shell, Eye, CrabBody)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=Sun)
    tonemap_aces(ColorHdr, sky=sky_color)
```

# Task 2: a reed bed in wind

```python
space("World", "Reed")

TAU = 6.2831853
PAPER = vec3(0.96, 0.92, 0.82)
INK = vec3(0.14, 0.12, 0.10)
WATER_INK = vec3(0.80, 0.77, 0.70)

@lobe
def ink(dark: vec3, cut: float, n: vec3, v: vec3, l: vec3) -> vec4:
    return vec4(dark if dot(n, l) < cut else PAPER, 0.0)

@kind
class Reed:
    root: Pt["World"]
    height: Meters
    lean: Rad
    stiffness: float
    phase: float
    bend: Rad          # written by the wind law
    clock: Sec

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        a1 = self.lean*0.3 + self.bend*0.2
        a2 = self.lean*0.7 + self.bend*0.8
        h = self.height
        mid = self.root + Vec["World"](0.5*h*sin(a1), 0.5*h*cos(a1), 0.0*m)
        tip = mid + Vec["World"](0.5*h*sin(a2), 0.5*h*cos(a2), 0.0*m)
        stalk_low = sd_capsule(p, self.root, mid, 0.010*m)
        stalk_high = sd_capsule(p, mid, tip, 0.006*m)
        seed = sd_sphere(p - (tip + Vec["World"](0.0, 0.03*m, 0.0)), 0.012*m)
        return smin_poly(smin_poly(stalk_low, stalk_high, 0.004*m), seed, 0.006*m)

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ink(INK, 0.0)

@kind
class Water:
    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return p.y

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ink(WATER_INK, 0.0)

@kind
class Sky:
    @query
    def sky(self, d: Dir["World"]) -> vec3:
        return PAPER*(1.0 - 0.1*abs(fbm2(vec2(d.x, d.z)*3.0)))

@kind
class Sun:
    dir: Dir["World"]

@comptime
def bed(n: int):
    rng = random.Random(42)
    for i in range(n):
        x = rng.gauss(0.0, 1.2)
        z = rng.gauss(0.0, 0.8)
        emit(Reed(root=Pt["World"](x*m, -0.1*m, z*m),
                  height=(0.9 + 0.5*rng.random())*m,
                  lean=(rng.random() - 0.5)*0.3*rad,
                  stiffness=0.5 + rng.random(),
                  phase=rng.random()*TAU,
                  bend=0.0*rad,
                  clock=0.0*s))

bed(200)
emit(Water())
emit(Sky())
emit(Sun(dir=Dir["World"](0.5, 0.8, 0.3)))

@law(writes=Reed)
def wind(r: Reed, dt: Sec):
    r.clock = r.clock + dt
    t = r.clock/s
    gust = 0.5 + 0.5*sin(0.9*r.root.x/m - 1.4*t)
    flutter = 0.15*sin(3.0*t + r.phase)
    r.bend = (0.9*gust + flutter)*rad / (1.0 + r.stiffness)

@schedule
def main():
    rows(Reed, Water, Sky, Sun)
    run_laws(wind)
    orbit(at=Pt["World"](0.0, 0.4, 0.0), radius=5*m, fov=35*deg)
    build_indirect(Water, Reed)
    with render_bundle(Vis64):
        draw_indirect(Water, Reed)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=Sun)
    tonemap_aces(ColorHdr, sky=Sky)
```

# Task 3: a heron's hop

```python
space("World", "Body", "Head", "Fish", "Ground")

PI = 3.14159265
LIMIT = 55*deg          # neck limit from the trunk heading
TURN_RATE = 1.5*rad/s   # trunk turn rate during the hop
HOP_T = 0.5*s
NECK_BASE = Pt["Body"](0.0, 0.92*m, 0.1*m)

@kind
class Fish:
    pos: Pt["World"]
    clock: Sec

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.7, 0.75, 0.8), 0.3, 0.0)

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return sd_sphere(p - self.pos, 0.06*m)

@kind
class Heron:
    origin: Vec["World"]
    trunk: Xform["Body", "World"]
    heading: Rad
    hop: Meters             # height of the hop, 0 on the ground
    hop_t: Sec              # time into the current hop
    neck_yaw: Rad           # head yaw relative to the trunk heading
    leftover: Rad           # gaze miss angle past the limit, 0 when held
    margin: Rad             # room left inside the limit, when held
    target: RowId["Fish"]

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        q = inv(self.trunk) @ p
        trunk = sd_capsule(q, Pt["Body"](0.0, 0.6*m, 0.0), Pt["Body"](0.0, 0.9*m, 0.0), 0.12*m)
        leg_l = sd_capsule(q, Pt["Body"](0.08*m, 0.6*m, 0.0), Pt["Body"](0.08*m, 0.0, 0.05*m), 0.01*m)
        leg_r = sd_capsule(q, Pt["Body"](-0.08*m, 0.6*m, 0.0), Pt["Body"](-0.08*m, 0.0, 0.05*m), 0.01*m)
        # S-shaped neck: shoulder, bend, head
        mid = Pt["Body"](0.06*m*sin(self.neck_yaw*0.5), 1.05*m, 0.12*m)
        head_c = Pt["Body"](0.25*m*sin(self.neck_yaw), 1.15*m, 0.2*m*cos(self.neck_yaw) + 0.1*m)
        neck_lo = sd_capsule(q, NECK_BASE, mid, 0.03*m)
        neck_hi = sd_capsule(q, mid, head_c, 0.02*m)
        head = sd_sphere(q - head_c, 0.05*m)
        beak_tip = head_c + Vec["Body"](0.2*m*sin(self.neck_yaw), -0.02*m, 0.2*m*cos(self.neck_yaw))
        beak = sd_capsule(q, head_c, beak_tip, 0.012*m)
        body = smin_poly(trunk, smin_poly(leg_l, leg_r, 0.02*m), 0.05*m)
        neck = smin_poly(neck_lo, neck_hi, 0.02*m)
        return smin_poly(smin_poly(body, neck, 0.03*m), smin_poly(head, beak, 0.01*m), 0.02*m)

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.55, 0.58, 0.60), 0.45, 0.0)

@kind
class Ground:
    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return p.y

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.42, 0.36, 0.28), 0.35, 0.0)

@kind
class Sun:
    dir: Dir["World"]

@comptime
def spawn():
    f = emit(Fish(pos=Pt["World"](2.0, 0.2, 3.0), clock=0.0*s))
    emit(Heron(origin=Vec["World"](0.0, 0.0, 0.0),
               trunk=rot_y(0.0*rad),
               heading=0.0*rad, hop=0.0*m, hop_t=0.0*s,
               neck_yaw=0.0*rad, leftover=0.0*rad, margin=LIMIT,
               target=f))
    emit(Ground())
    emit(Sun(dir=Dir["World"](0.4, 0.8, 0.3)))

spawn()

@law(writes=Fish)
def fish_swim(f: Fish, dt: Sec):
    f.clock = f.clock + dt
    t = f.clock/s
    f.pos = Pt["World"](2.0*cos(0.3*t), 0.2, 3.0 + 1.0*sin(0.2*t))

# The body goes first: it hops and turns while the neck is at its limit
@law(writes=Heron)
def body_hop(h: Heron, dt: Sec, fish: Table["Fish"]):
    f = fish[h.target]
    stuck = h.leftover > 0*rad
    if stuck:
        h.hop_t = h.hop_t + dt
        if h.hop_t > HOP_T:
            h.hop_t = 0*s
        h.hop = 0.12*m*sin(PI*(h.hop_t/HOP_T))
    else:
        h.hop_t = 0*s
        h.hop = 0*m
    if h.hop > 0.005*m:
        aim = atan2((inv(h.trunk) @ f.pos - Pt["Body"](0.0, 0.0, 0.0)).x,
                    (inv(h.trunk) @ f.pos - Pt["Body"](0.0, 0.0, 0.0)).z)*rad
        step = TURN_RATE*dt
        if aim > step:
            h.heading = h.heading + step
        elif aim < -step:
            h.heading = h.heading - step
        else:
            h.heading = h.heading + aim
    h.trunk = translate(h.origin + Vec["World"](0.0, h.hop, 0.0)) @ rot_y(h.heading)

# Then the neck and head: the head aims at the fish in world space, clamped to the limit
@law(writes=Heron)
def neck_head(h: Heron, dt: Sec, fish: Table["Fish"]):
    f = fish[h.target]
    base = h.trunk @ NECK_BASE
    d = f.pos - base
    want = atan2(d.x, d.z)*rad - h.heading
    if want > LIMIT:
        h.neck_yaw = LIMIT
        h.leftover = want - LIMIT
        h.margin = 0*rad
    elif want < -LIMIT:
        h.neck_yaw = -LIMIT
        h.leftover = -want - LIMIT
        h.margin = 0*rad
    else:
        h.neck_yaw = want
        h.leftover = 0*rad
        h.margin = LIMIT - abs(want)

@constraint
def gaze_held(h: Heron) -> Verdict[Rad]:
    if h.leftover > 0*rad:
        return Missed(h.leftover)
    return Held(h.margin)

@constraint
def head_above_hips(h: Heron) -> Verdict[Meters]:
    head_p = h.trunk @ Pt["Body"](0.25*m*sin(h.neck_yaw), 1.15*m, 0.2*m*cos(h.neck_yaw) + 0.1*m)
    hips = h.trunk @ Pt["Body"](0.0, 0.6*m, 0.0)
    margin = head_p.y - hips.y
    if margin < 0*m:
        return Missed(-margin)
    return Held(margin)

@schedule
def main():
    rows(Fish, Heron, Ground, Sun)
    run_laws(fish_swim)
    run_laws(body_hop, neck_head)
    check(gaze_held)
    check(head_above_hips)
    orbit(at=Pt["World"](0.0, 0.6, 0.0), radius=3.0*m, fov=40*deg)
    build_indirect(Ground, Heron, Fish)
    with render_bundle(Vis64):
        draw_indirect(Ground, Heron, Fish)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=Sun)
    tonemap_aces(ColorHdr, sky=sky_color)
```
