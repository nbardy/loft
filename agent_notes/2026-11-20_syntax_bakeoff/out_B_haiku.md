# Syntax bakeoff, card B, Haiku 5.5

## Task 1: hermit crab

```loft
// Task 1: a hermit crab on wet sand.

space Body, Leg, Claw, Face, Shell;

kind CrabBody {
    row { pose: Xform[Body, World], com: Pt[World], vel: Vec[World], head: Pt[Body], }
}

kind CrabLeg {
    row { parent: Row[CrabBody], foot: Pt[World], femur: Meters, tibia: Meters, side: f32, }
}

// Spiral spine in Shell space: radius grows along the whorl, the spine climbs.
fn spine(u: Unit) -> Pt[Shell] {
    let a = 6.0 * 6.2831853 * u;
    let r = 0.02m + 0.04m * u;
    return Pt[Shell](r * cos(a), 0.05m * u, r * sin(a));
}

scan whorl(u: Unit) @schedule(bake_lut, 256u) -> Table[Basis[Shell]] {
    init: frame_seed(spine(0), normalize(diff(spine, u)(0)), Dir[Shell](0, 1, 0));
    step(last, u_i): transport_step(last, spine(u_i), normalize(diff(spine, u)(u_i)));
}

// Ribbed tube swept around the spine. The rib modulates radius along the whorl.
fn shell_point(u: Unit, v: Unit) -> Pt[Shell] {
    let f = sample_c2(whorl, u);
    let rib = 1.0 + 0.08 * sin(2.0 * 3.14159265 * 14.0 * u);
    let r = (0.008m + 0.012m * u) * rib;
    let a = 6.2831853 * v;
    return f.origin + r * (cos(a) * f.normal + sin(a) * f.binormal);
}

// Crusher claw: rounded palm, fixed finger, hinged moving finger with serrations.
// The opening angle and scale are data; both claws use this one definition.
fn claw_sdf(p: Pt[Claw], open: Rad, k: f32) -> Meters {
    let palm = sd_round_box(p, Vec[Claw](0.05, 0.035, 0.04) * k, 0.015m * k);
    let fixed = sd_capsule(p, Pt[Claw](0.03, 0.0, 0.0) * k, Pt[Claw](0.13, 0.0, 0.015) * k, 0.010m * k);
    let hinge = Pt[Claw](0.03, 0.0, 0.0) * k;
    let d = p - hinge;
    let c = cos(open);
    let s = sin(open);
    let q = hinge + Vec[Claw](c * d.x - s * d.z, d.y, s * d.x + c * d.z);
    let teeth = 0.003m * k * sin(q.x / (0.01m * k) * 3.14159);
    let moving = sd_capsule(q, hinge, hinge + Vec[Claw](0.10, 0.0, 0.0) * k, 0.009m * k) + teeth;
    return smin_poly(palm, smin_poly(fixed, moving, 0.008m * k), 0.02m * k);
}

fn add_claw(w: World, at: Attachment, open: Rad, k: f32) {
    add(w, field(box(Pt[Claw](-0.2, -0.2, -0.2), Pt[Claw](0.2, 0.2, 0.2)),
                 |p| claw_sdf(p, open, k)), chitin, at);
}

// One leg: femur and tibia capsules, the knee bent by the tibia length.
fn leg_sdf(p: Pt[Leg], femur: Meters, tibia: Meters) -> Meters {
    let knee = Pt[Leg](femur, 0m, 0m);
    let ankle = knee + Vec[Leg](0.6 * tibia, -0.8 * tibia, 0m);
    let upper = sd_capsule(p, Pt[Leg](0m, 0m, 0m), knee, 0.07m);
    let lower = sd_capsule(p, knee, ankle, 0.04m);
    return smin_poly(upper, lower, 0.02m);
}

// Layered wet chitin. Fine noise varies roughness in the material, not in the distance field.
fn chitin(h: Hit) -> Bsdf {
    let rough = 0.25 + 0.15 * fbm2(h.p.xz * 120.0);
    return coat(sss(ggx(rgb(0.72, 0.2, 0.06), rough, 0.0), rgb(0.9, 0.35, 0.12), 0.01m), 0.7, 0.05, 1.34);
}

fn wet_sand(h: Hit) -> Bsdf {
    return coat(ggx(rgb(0.42, 0.35, 0.27), 0.6, 0.0), 0.8, 0.05, 1.33);
}

fn eye_white(h: Hit) -> Bsdf {
    return coat(ggx(rgb(0.9, 0.88, 0.8), 0.2, 0.0), 0.6, 0.05, 1.4);
}

fn pupil_black(h: Hit) -> Bsdf {
    return ggx(rgb(0.02, 0.02, 0.03), 0.1, 0.0);
}

// Eye pair from one mirrored definition. The map sends the left eye to the right, so each
// eye's world position and aim come out different. Both aim at the same world target.
fn add_eye_pair(w: World, body: Xform[Body, World], crab: Row[CrabBody], target: Pt[World]) {
    map e = mirror_x(p) |> translate(Vec(-0.15, -0.72, -0.42));
    let c = Pt['e](0.03, 0.30, 0.07);
    let eye_w = body * (e.to_parent * c);
    let aim = normalize(target - eye_w);
    add(w, sphere(0.085m), eye_white, attachment(crab, |r| rigid[Face, World](eye_w - Pt[World](0m, 0m, 0m))));
    add(w, sphere(0.03m), pupil_black,
        attachment(crab, |r| rigid[Face, World](eye_w + 0.075m * aim - Pt[World](0m, 0m, 0m))));
}

// Motion law: the leg follows the body's centre of mass, stepping when it falls far behind.
pub law crab_step(leg: CrabLeg, bodies: Table[CrabBody], dt: Sec) -> CrabLeg {
    let b = bodies[leg.parent];
    var next = leg;
    let cp = b.com + b.vel * 0.28s;
    if distance(leg.foot, cp) > 0.5m { next.foot = Pt[World](cp.x, 0, cp.z); }
    return next;
}

pub constraint upright(b: CrabBody) -> Verdict[Meters] {
    if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
    let margin = (b.pose * b.head).y - 0.15m;
    if margin < 0m { return Verdict::Missed(-margin); }
    return Verdict::Held(margin);
}

pub comptime fn scene() -> Scene {
    let w = world();
    let body_pose = rigid[Body, World](Vec[World](0, 0.04, 0));
    let crab = emit CrabBody { pose: body_pose, com: Pt[World](0, 0.04, 0), vel: Vec[World](0, 0, 0), head: Pt[Body](0, 0.12, 0.10) };

    // Sand plane.
    add(w, plane(Dir[World](0, 1, 0), 0m), wet_sand);

    // Shell, sitting on the back of the body.
    let shell_mount = rigid[Shell, Body](Vec[Body](0.0, 0.10, -0.05));
    add(w, surface(|uv| shell_point(uv.x, uv.y), grid(96u, 24u)), chitin,
        attachment(crab, |r| r.pose * shell_mount));

    // Crusher claw (large, opening 0.6 rad) and feeder claw (smaller, other side, opening 0.4 rad).
    add_claw(w, attachment(crab, |r| r.pose * rigid[Claw, Body](Vec[Body](0.12, 0.06, 0.12))), 0.6rad, 1.0);
    add_claw(w, attachment(crab, |r| r.pose * rigid[Claw, Body](Vec[Body](-0.08, 0.05, 0.10))), 0.4rad, 0.6);

    // Twelve legs, six per side, one kind, emitted in a loop.
    comptime for i in 0u..12u {
        let side = select(-1.0, 1.0, i % 2u == 1u);
        let k = i / 2u;
        let femur = 0.40m + 0.05m * f32(k);
        let tibia = 0.25m + 0.03m * f32(k);
        emit CrabLeg { parent: crab, foot: Pt[World](side * 0.2, 0, 0.3 * f32(k)), femur: femur, tibia: tibia, side: side };
        let at = attachment(crab, |r| r.pose * leg_socket(k, side));
        add(w, field(box(Pt[Leg](-0.1, -0.1, -0.1), Pt[Leg](0.8, 0.1, 0.1)),
                     |p| leg_sdf(p, femur, tibia)), wet_sand, at);
    }

    // Eyes on stalks.
    let target = Pt[World](0.6, 0.15, 1.2 + 0.2 * sin(0.05 * time() / 1.0s));
    add_eye_pair(w, body_pose, crab, target);

    add_sun(w, normalize(Vec[World](-3, 5, 2)), rgb(3, 3, 2.5));
    environment(w, |d| rgb(0.4, 0.5, 0.6));
    return frame(w, look_at(Pt[World](0.6, 0.35, 1.8), Pt[World](0.2, 0.05, 0.6), Vec[World](0, 1, 0), 0.6rad), main);
}

pub fn main(w: World, c: Camera) -> Schedule {
    let s = plan(w, c);
    run_laws(s, w, [crab_step]);
    check(s, w, upright);
    let n = clear_counts(s, w);
    let v = record(s, w, c, cull(s, w, c, n), vis64());
    let hdr = deferred(s, w, v, classify(s, v, n), sun_and_sky);
    return output(s, hdr, aces);
}
```

## Task 2: reed bed in wind

```loft
// Task 2: a reed bed in wind. Two hundred reeds, one kind, flat two-ink look.

space Stalk;

// One row per reed. Data varies per reed; the law updates clock and sway each tick.
kind Reed {
    row {
        root: Pt[World],
        height: Meters,
        lean: f32,
        stiffness: f32,
        phase: f32,
        clock: Sec,
        sway: Meters,
    }
}

// Deterministic jitter for the comptime cluster. Returns a value in [0, 1).
fn jitter(i: u32, k: f32) -> f32 {
    return fract(sin(f32(i) * 12.9898 + k * 78.233) * 43758.5453);
}

// Thin tapered stalk with a seed head. The tip offset is `sway`; the bend grows toward the tip.
fn reed_sdf(p: Pt[Stalk], r: Row[Reed]) -> Meters {
    let tip_x = r.sway + r.lean * r.height;
    let mid = Pt[Stalk](0.25 * tip_x, 0.5 * r.height, 0m);
    let tip = Pt[Stalk](tip_x, r.height, 0m);
    let lower = sd_capsule(p, Pt[Stalk](0m, 0m, 0m), mid, 0.0055m);
    let upper = sd_capsule(p, mid, tip, 0.0030m);
    let head = sd_capsule(p, Pt[Stalk](tip_x, r.height - 0.10m, 0m), tip, 0.012m);
    return smin_poly(smin_poly(lower, upper, 0.002m), head, 0.004m);
}

// Motion law: each reed reads only its own data and the clock. A gust travels across the bed
// along x; stiffer reeds bend less.
pub law reed_bed(r: Reed, dt: Sec) -> Reed {
    var next = r;
    next.clock = r.clock + dt;
    let gust = sin(r.phase + 1.3 * next.clock / 1s - 0.9 * r.root.x / 1m);
    let amp = r.height * 0.4 / (1.0 + r.stiffness);
    next.sway = amp * (0.5 + 0.5 * gust);
    return next;
}

// Flat two-ink look. Lit sides take paper; shadowed sides take dark ink with a hard edge.
// Water and sky take a faint second ink.
fn reed_look(w: World, h: Hit, wo: DirW, k: Key) -> Rgb {
    let paper = rgb(0.96, 0.92, 0.82);
    let dark = rgb(0.1, 0.1, 0.12);
    let faint = rgb(0.86, 0.82, 0.70);
    let src = light(w, h.p, k);
    let lit = select(paper, dark, dot(h.n, src.wi) < 0.4);
    return select(lit, faint, h.p.y < 0.01m);
}

pub comptime fn scene() -> Scene {
    let w = world();

    // Two hundred reeds in a loose, irregular cluster.
    comptime for i in 0u..200u {
        let x = -1.6m + 3.2m * jitter(i, 1.0);
        let z = 0.4m + 1.6m * jitter(i, 2.0);
        let height = 0.9m + 0.6m * jitter(i, 3.0);
        let lean = 0.2 * (jitter(i, 4.0) - 0.5);
        let stiffness = jitter(i, 5.0);
        let phase = 6.2831853 * jitter(i, 6.0);
        let r = emit Reed { root: Pt[World](x, 0m, z), height: height, lean: lean, stiffness: stiffness,
                            phase: phase, clock: 0s, sway: 0m };
        add(w, field(box(Pt[Stalk](-0.1m, -0.1m, -0.1m), Pt[Stalk](0.1m, 1.6m, 0.1m)),
                     |p| reed_sdf(p, r)), reed_ink, attachment(r, |row| rigid[Stalk, World](row.root - Pt[World](0m, 0m, 0m))));
    }

    // Shallow water: a plane. Its look is the faint second ink through reed_look.
    add(w, plane(Dir[World](0, 1, 0), 0m), reed_ink);

    add_sun(w, normalize(Vec[World](-2, 4, 3)), rgb(1, 1, 1));
    environment(w, |d| rgb(0.96, 0.92, 0.82));
    return frame(w, look_at(Pt[World](0, 0.8, -1.5), Pt[World](0, 0.5, 1.0), Vec[World](0, 1, 0), 0.7rad), main);
}

fn reed_ink(h: Hit) -> Bsdf {
    return ggx(rgb(0.9, 0.9, 0.9), 1.0, 0.0);
}

pub fn main(w: World, c: Camera) -> Schedule {
    let s = plan(w, c);
    run_laws(s, w, [reed_bed]);
    let n = clear_counts(s, w);
    let v = record(s, w, c, cull(s, w, c, n), vis64());
    let hdr = deferred(s, w, v, classify(s, v, n), reed_look);
    return output(s, hdr, aces);
}
```

## Task 3: heron's hop

```loft
// Task 3: a heron's hop. Gaze lock first, neck limit, then the body turns in a small hop.

space Trunk, Neck, Head, Leg;

kind Fish {
    row { pos: Pt[World], t: Sec, }
}

// Heron row. The neck and head state lives here so the constraints can read it.
kind Heron {
    row {
        pos: Pt[World],
        t: Sec,
        yaw: Rad,
        vy: f32,
        pose: Xform[Trunk, World],
        head_pose: Xform[Head, World],
        head_yaw: Rad,
        gaze_err: Rad,
        slack: Rad,
    }
}

// The fish moves along a curve at the water's edge, in world metres.
fn fish_at(t: Sec) -> Pt[World] {
    let a = 0.35 * t / 1s;
    return Pt[World](1.6m + 0.5m * cos(a), 0.05m, 1.2m + 0.6m * sin(a));
}

// Trunk pose: translate to the heron's position, then turn it about the vertical axis.
fn trunk_pose(p: Pt[World], yaw: Rad) -> Xform[Trunk, World] {
    return rigid[Trunk, World](p - Pt[World](0m, 0m, 0m)) * rotate(Dir[World](0, 1, 0), yaw);
}

// Head frame: the neck joint sits on top of the trunk; the head turns by `yaw` at that joint.
fn head_frame(pose: Xform[Trunk, World], yaw: Rad) -> Xform[Head, World] {
    return pose * rigid[Head, Trunk](Vec[Trunk](0, 0.55m, 0)) * rotate(Dir[Head](0, 1, 0), yaw);
}

pub law fish_swim(f: Fish, dt: Sec) -> Fish {
    var next = f;
    next.t = f.t + dt;
    next.pos = fish_at(next.t);
    return next;
}

// Body law, runs first. While the neck is at its limit, the heron hops: it leaves the ground,
// turns its trunk toward the fish, and lands.
pub law body_law(h: Heron, dt: Sec) -> Heron {
    var next = h;
    next.t = h.t + dt;
    let fish = fish_at(next.t);
    let to = fish - h.pos;
    let want: Rad = atan2(to.x / 1m, to.z / 1m) * 1rad;
    let err = want - h.yaw;
    let blocked = abs(h.gaze_err) > 0rad;
    let on_ground = h.pos.y <= 0m;
    var vy = h.vy;
    if blocked && on_ground { vy = 1.6; }
    vy = vy - 9.81 * dt / 1s;
    var y = h.pos.y + vy * m_per_s_dt(dt);
    if y <= 0m { y = 0m; vy = 0.0; }
    next.vy = vy;
    next.pos = Pt[World](h.pos.x, y, h.pos.z);
    if blocked {
        let step = clamp(err, -1.5rad * dt / 1s, 1.5rad * dt / 1s);
        next.yaw = h.yaw + step;
    }
    next.pose = trunk_pose(next.pos, next.yaw);
    return next;
}

// Neck and head law. The gaze lock is computed each tick in world space. The head turns at most
// 1.1 rad from the trunk heading. Beyond that it stops at the limit and the leftover angle is
// kept as gaze_err. Inside the limit, slack is the margin.
pub law neck_head(h: Heron) -> Heron {
    var next = h;
    let fish = fish_at(h.t);
    let to = fish - h.pos;
    let rel: Rad = atan2(to.x / 1m, to.z / 1m) * 1rad - h.yaw;
    let lim = 1.1rad;
    let held = clamp(rel, -lim, lim);
    next.head_yaw = held;
    next.gaze_err = rel - held;
    next.slack = lim - abs(rel);
    next.head_pose = head_frame(next.pose, held);
    return next;
}

// Gaze constraint. It reports missed by the leftover angle when the head is at its limit.
// It reports held only when the target is inside the limit.
pub constraint gaze(h: Heron) -> Verdict[Rad] {
    if abs(h.gaze_err) > 0rad { return Verdict::Missed(abs(h.gaze_err)); }
    return Verdict::Held(h.slack);
}

// Head stays above the hips throughout.
pub constraint head_above_hips(h: Heron) -> Verdict[Meters] {
    let head_y = (h.head_pose * Pt[Head](0m, 0m, 0m)).y;
    let hip_y = (h.pose * Pt[Trunk](0m, 0.45m, 0m)).y;
    let margin = head_y - hip_y;
    if margin < 0m { return Verdict::Missed(-margin); }
    return Verdict::Held(margin);
}

pub comptime fn scene() -> Scene {
    let w = world();
    let hr = emit Heron { pos: Pt[World](0m, 0m, 0m), t: 0s, yaw: 0rad, vy: 0.0,
                          pose: trunk_pose(Pt[World](0m, 0m, 0m), 0rad),
                          head_pose: head_frame(trunk_pose(Pt[World](0m, 0m, 0m), 0rad), 0rad),
                          head_yaw: 0rad, gaze_err: 0rad, slack: 1.1rad };
    let fr = emit Fish { pos: fish_at(0s), t: 0s };

    // Ground.
    add(w, plane(Dir[World](0, 1, 0), 0m), ground_mat);

    // Trunk, a capsule along the body.
    add(w, field(box(Pt[Trunk](-0.2m, -0.2m, -0.3m), Pt[Trunk](0.2m, 0.8m, 0.3m)),
                 |p| sd_capsule(p, Pt[Trunk](0m, 0.1m, 0m), Pt[Trunk](0m, 0.6m, 0m), 0.14m)),
        plumage, attachment(hr, |r| r.pose));

    // S-shaped neck: three capsules along the trunk's forward side.
    add(w, field(box(Pt[Neck](-0.1m, -0.1m, -0.1m), Pt[Neck](0.1m, 0.8m, 0.4m)),
                 |p| smin_poly(sd_capsule(p, Pt[Neck](0m, 0m, 0m), Pt[Neck](0m, 0.25m, 0.10m), 0.035m),
                               sd_capsule(p, Pt[Neck](0m, 0.25m, 0.10m), Pt[Neck](0m, 0.50m, 0.05m), 0.030m),
                               0.02m)),
        plumage, attachment(hr, |r| r.pose * rigid[Neck, Trunk](Vec[Trunk](0, 0.55m, 0))));

    // Head sphere and beak on the head's forward (+z) axis.
    add(w, sphere(0.07m), plumage, attachment(hr, |r| r.head_pose));
    add(w, field(box(Pt[Head](-0.02m, -0.02m, -0.02m), Pt[Head](0.02m, 0.02m, 0.3m)),
                 |p| sd_capsule(p, Pt[Head](0m, 0m, 0.05m), Pt[Head](0m, 0m, 0.24m), 0.014m)),
        beak_mat, attachment(hr, |r| r.head_pose));

    // Two legs.
    add(w, field(box(Pt[Leg](-0.1m, -0.6m, -0.1m), Pt[Leg](0.1m, 0.1m, 0.1m)),
                 |p| sd_capsule(p, Pt[Leg](0m, 0m, 0m), Pt[Leg](0m, -0.45m, 0m), 0.02m)),
        leg_mat, attachment(hr, |r| r.pose * rigid[Leg, Trunk](Vec[Trunk](0.08m, 0.0m, 0m))));
    add(w, field(box(Pt[Leg](-0.1m, -0.6m, -0.1m), Pt[Leg](0.1m, 0.1m, 0.1m)),
                 |p| sd_capsule(p, Pt[Leg](0m, 0m, 0m), Pt[Leg](0m, -0.45m, 0m), 0.02m)),
        leg_mat, attachment(hr, |r| r.pose * rigid[Leg, Trunk](Vec[Trunk](-0.08m, 0.0m, 0m))));

    // Fish: the gaze target.
    add(w, sphere(0.04m), silver_mat, attachment(fr, |r| rigid[Fish, World](r.pos - Pt[World](0m, 0m, 0m))));

    add_sun(w, normalize(Vec[World](-3, 5, 2)), rgb(3, 3, 2.5));
    environment(w, |d| rgb(0.4, 0.5, 0.6));
    return frame(w, look_at(Pt[World](0.6, 0.9, -1.2), Pt[World](1.0, 0.5, 0.8), Vec[World](0, 1, 0), 0.6rad), main);
}

fn ground_mat(h: Hit) -> Bsdf { return ggx(rgb(0.5, 0.45, 0.35), 0.9, 0.0); }
fn plumage(h: Hit) -> Bsdf { return ggx(rgb(0.6, 0.6, 0.62), 0.5, 0.0); }
fn beak_mat(h: Hit) -> Bsdf { return ggx(rgb(0.9, 0.7, 0.2), 0.3, 0.0); }
fn leg_mat(h: Hit) -> Bsdf { return ggx(rgb(0.7, 0.5, 0.3), 0.6, 0.0); }
fn silver_mat(h: Hit) -> Bsdf { return ggx(rgb(0.8, 0.82, 0.86), 0.2, 1.0); }

pub fn main(w: World, c: Camera) -> Schedule {
    let s = plan(w, c);
    run_laws(s, w, [fish_swim, body_law, neck_head]);
    check(s, w, gaze);
    check(s, w, head_above_hips);
    let n = clear_counts(s, w);
    let v = record(s, w, c, cull(s, w, c, n), vis64());
    let hdr = deferred(s, w, v, classify(s, v, n), sun_and_sky);
    return output(s, hdr, aces);
}
```
