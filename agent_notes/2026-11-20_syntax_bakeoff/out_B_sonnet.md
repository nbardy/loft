## Task 1

```
import std::render::*;

space Body, Leg, Claw, Face, Shell;

kind CrabBody { row { pose: Xform[Body, World], com: Pt[World], vel: Vec[World], head: Pt[Body], } }
kind CrabLeg  { row { parent: Row[CrabBody], foot: Pt[World], } }

fn tau() -> f32 { return 6.2831853; }

fn drift() -> Pt[World] {
    let t = time() / 1s;
    return Pt[World](0.6 * sin(0.21 * t), 0.35, 1.2 + 0.4 * cos(0.13 * t));
}

fn root() -> Xform[World, World] { return Xform[World, World]::identity(); }

fn leg_socket(k: u32, side: f32) -> Xform[Leg, Body] {
    return rigid[Leg, Body](Vec[Body](side * 0.22, 0.0, 0.18 - 0.11 * f32(k)));
}
fn claw_socket(side: f32) -> Xform[Claw, Body] {
    return rigid[Claw, Body](Vec[Body](side * 0.28, 0.05, 0.42));
}
fn face_socket() -> Xform[Face, Body] {
    return rigid[Face, Body](Vec[Body](0.0, 0.14, 0.40));
}
fn shell_socket() -> Xform[Shell, Body] {
    return rigid[Shell, Body](Vec[Body](0.0, 0.18, -0.30));
}

// ---------- shell: scanned frames along a spiral spine ----------
fn spine(u: Unit) -> Pt[Shell] {
    let a = 18.0 * u;
    let r = 0.015 * exp(0.13 * a);
    return Pt[Shell](r * cos(a), 0.012 * a, r * sin(a));
}
fn tube_r(u: Unit) -> f32 { return 0.004 + 0.045 * exp(2.3 * u) / exp(2.3); }

scan whorl(u: Unit) @schedule(bake_lut, 256u) -> Table[Basis[Shell]] {
    init: frame_seed(spine(0), normalize(diff(spine, u)(0)), Dir[Shell](0, 1, 0));
    step(last, u_i): transport_step(last, spine(u_i), normalize(diff(spine, u)(u_i)));
}

fn shell_point(uv: Vec2) -> Pt[Shell] {
    let f = sample_c2(whorl, uv.x);
    let phi = tau() * uv.y;
    let rib = 1.0 + 0.10 * sin(160.0 * uv.x);
    let r = tube_r(uv.x) * rib;
    return f.origin + (f.normal * cos(phi) + f.binormal * sin(phi)) * r;
}

// ---------- claw: one definition, differing only in data ----------
fn claw_sdf(p: Pt[Claw], open: Rad, s: f32) -> Meters {
    let palm = sd_round_box(p, Vec[Claw](0.16 * s, 0.11 * s, 0.09 * s), 0.03 * s);
    let fixed_finger = sd_capsule(p, Pt[Claw](0.10 * s, -0.06 * s, 0), Pt[Claw](0.46 * s, -0.04 * s, 0), 0.05 * s);
    map h = translate(Vec(-0.10 * s, 0.06 * s, 0)) |> rotate(Vec(0, 0, 1), open);
    let teeth = 0.010 * s * max(sin(70.0 * h.p.x / s), 0.0);
    let moving = sd_capsule(h.p, Pt['h](0, 0, 0), Pt['h](0.46 * s, 0, 0), 0.045 * s) - teeth;
    return smin_poly(smin_poly(palm, fixed_finger, 0.03 * s), moving, 0.02 * s);
}

comptime fn add_claw(w: World, crab: Row[CrabBody], side: f32, open: Rad, size: f32) {
    let at = attachment(crab, |r| r.pose * claw_socket(side));
    add(w, field(box(Pt[Claw](-0.3 * size, -0.4 * size, -0.2 * size), Pt[Claw](0.7 * size, 0.4 * size, 0.2 * size)),
                 |p| claw_sdf(p, open, size)), wet, at);
}

// ---------- eyes: one mirrored definition ----------
fn eye_stalk(p: Pt[Face], xf: Xform[Face, World]) -> Meters {
    map e = mirror_x(p) |> translate(Vec(-0.15, -0.72, -0.42));
    let c = Pt['e](0.03, 0.30, 0.07);
    let stalk = sd_capsule(e.p, Pt['e](0.03, 0.0, 0.0), c, 0.025m);
    let globe = length(e.p - c) - 0.085m;
    return min(stalk, globe);
}
fn eye_pupil(p: Pt[Face], xf: Xform[Face, World], target: Pt[World]) -> Meters {
    map e = mirror_x(p) |> translate(Vec(-0.15, -0.72, -0.42));
    let c = Pt['e](0.03, 0.30, 0.07);
    let eye_w = xf * (e.to_parent * c);
    let look = e.from_parent.dir(inverse(xf) * normalize(target - eye_w));
    return length(e.p - (c + 0.075m * look)) - 0.03m;
}

// ---------- materials ----------
fn wet(h: Hit) -> Bsdf {
    let n = fbm2(vec2(h.p.x + h.p.z, h.p.y + h.p.z) * 90.0);
    let rough = 0.22 + 0.22 * n;
    return coat(sss(ggx(rgb(0.6, 0.14, 0.05), rough, 0.0), rgb(0.9, 0.3, 0.1), 0.02m), 0.7, 0.07, 1.34);
}
fn pupil_mat(h: Hit) -> Bsdf { return ggx(rgb(0.02, 0.02, 0.02), 0.1, 0.0); }
fn sand(h: Hit) -> Bsdf {
    return coat(ggx(rgb(0.42, 0.34, 0.24), 0.7, 0.0), 0.5, 0.2, 1.33);
}

// ---------- laws ----------
pub law body_law(b: CrabBody, dt: Sec) -> CrabBody { return b; }

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

// ---------- build ----------
pub comptime fn scene() -> Scene {
    let w = world();
    let crab_xf = rigid[Body, World](Vec[World](0, 0.05, 0));
    let crab = emit CrabBody {
        pose: crab_xf, com: Pt[World](0, 0.05, 0), vel: Vec[World](0, 0, 0), head: Pt[Body](0, 0.3, 0.4),
    };

    // sand
    add(w, plane(Dir[World](0, 1, 0), 0m), sand, attachment(crab, |r| root()));

    // body
    add(w, sphere(0.2m), wet, attachment(crab, |r| r.pose * rigid[Body, Body](Vec[Body](0, 0.1, 0.1))));

    // shell
    add(w, surface(|uv| shell_point(uv), grid(512u, 48u)), wet, attachment(crab, |r| r.pose * shell_socket()));

    // legs: six per side
    comptime for i in 0u..12u {
        let side = select(-1.0, 1.0, i % 2u == 1u);
        let k = i / 2u;
        emit CrabLeg { parent: crab, foot: Pt[World](side * 0.5, 0, 0.2 - 0.12 * f32(k)) };
        let at = attachment(crab, |r| r.pose * leg_socket(k, side));
        let femur = 0.14m + 0.02m * f32(k);
        let tibia = 0.18m + 0.015m * f32(k);
        add(w, field(box(Pt[Leg](-0.5, -0.4, -0.1), Pt[Leg](0.5, 0.1, 0.1)),
                     |p| smin_poly(
                         sd_capsule(p, Pt[Leg](0, 0, 0), Pt[Leg](side * femur, 0.05, 0), 0.02m),
                         sd_capsule(p, Pt[Leg](side * femur, 0.05, 0), Pt[Leg](side * (femur + 0.4 * tibia), -tibia, 0), 0.014m),
                         0.01m)), wet, at);
    }

    // claws: same definition, different data
    add_claw(w, crab, -1.0, 0.55rad, 1.0);   // crusher
    add_claw(w, crab, 1.0, 0.25rad, 0.45);   // feeder

    // eyes
    let fxf = crab_xf * face_socket();
    let face_at = attachment(crab, |r| r.pose * face_socket());
    let bx = box(Pt[Face](-0.4, -0.1, -0.3), Pt[Face](0.4, 0.5, 0.3));
    add(w, field(bx, |p| eye_stalk(p, fxf)), wet, face_at);
    add(w, field(bx, |p| eye_pupil(p, fxf, drift())), pupil_mat, face_at);

    add_sun(w, normalize(Vec[World](-3, 5, 2)), rgb(3, 3, 2.5));
    environment(w, |d| rgb(0.4, 0.5, 0.6));

    let eye = Pt[World](0.9, 0.55, 1.4);
    let target = Pt[World](0, 0.2, 0);
    let up = Dir[World](0, 1, 0);
    return frame(w, look_at(eye, target, up, 0.6rad), main);
}

pub fn main(w: World, c: Camera) -> Schedule {
    let s = plan(w, c);
    run_laws(s, w, [body_law, crab_step]);
    check(s, w, upright);
    let n = clear_counts(s, w);
    let v = record(s, w, c, cull(s, w, c, n), vis64());
    let hdr = deferred(s, w, v, classify(s, v, n), sun_and_sky);
    return output(s, hdr, aces);
}
```

## Task 2

```
import std::render::*;

space Reed;

kind Reed {
    row { xf: Xform[Reed, World], height: f32, lean: f32, stiffness: f32, phase: f32, }
}

kind Ground { row { } }

fn rnd(i: u32, k: u32) -> f32 {
    var x = i * 747796405u + k * 2891336453u;
    x = (x ^ (x >> 16u)) * 2246822519u;
    x = x ^ (x >> 13u);
    return f32(x & 16777215u) / 16777216.0;
}

fn root() -> Xform[World, World] { return Xform[World, World]::identity(); }

// bend offset (metres) of a reed at height y: grows toward the tip, a gust travels across the bed in +x
fn bend(y: f32, height: f32, lean: f32, stiffness: f32, phase: f32, bx: f32, t: f32) -> f32 {
    let k = (y / height) * (y / height);
    let gust = 0.5 + 0.5 * sin(0.9 * bx - 1.6 * t);
    let flutter = 0.7 + 0.3 * sin(2.3 * t + phase);
    return k * height * (lean + 0.35 * gust * flutter / stiffness);
}

fn stalk(p: Pt[Reed], height: f32, lean: f32, stiffness: f32, phase: f32, bx: f32) -> Meters {
    let t = time() / 1s;
    let y = clamp(p.y, 0.0, height);
    let off = bend(y, height, lean, stiffness, phase, bx, t);
    let r = 0.011 * (1.0 - 0.55 * y / height);
    let d = length(vec2(p.x - off, p.z)) - r;
    return max(d, max(-p.y, p.y - height));
}

fn seed_head(p: Pt[Reed], height: f32, lean: f32, stiffness: f32, phase: f32, bx: f32) -> Meters {
    let t = time() / 1s;
    let top = bend(height, height, lean, stiffness, phase, bx, t);
    let a = Pt[Reed](top, height - 0.02, 0);
    let b = Pt[Reed](top + 0.02 * lean, height + 0.13, 0);
    return sd_capsule(p, a, b, 0.022m);
}

fn paper() -> Rgb { return rgb(0.93, 0.87, 0.74); }
fn ink_a() -> Rgb { return rgb(0.13, 0.17, 0.20); }
fn ink_b() -> Rgb { return rgb(0.45, 0.50, 0.52); }

fn reed_ink(w: World, h: Hit, wo: DirW, k: Key) -> Rgb {
    let src = light(w, h.p, k);
    return select(paper(), ink_a(), dot(h.n, src.wi) < 0.35);
}
fn head_ink(w: World, h: Hit, wo: DirW, k: Key) -> Rgb {
    let src = light(w, h.p, k);
    return select(paper(), rgb(0.28, 0.12, 0.08), dot(h.n, src.wi) < 0.35);
}
fn water_ink(w: World, h: Hit, wo: DirW, k: Key) -> Rgb {
    let band = fract(h.p.z * 6.0 + 0.4 * sin(h.p.x * 1.7));
    return select(paper(), mix(paper(), ink_b(), 0.18), band < 0.06);
}

pub comptime fn scene() -> Scene {
    let w = world();
    let ground = emit Ground { };
    add(w, plane(Dir[World](0, 1, 0), 0m), water_ink, attachment(ground, |r| root()));
    comptime for i in 0u..200u {
        let ang = 6.2831853 * rnd(i, 1u);
        let rad = 1.8 * pow(rnd(i, 2u), 0.7);
        let x = rad * cos(ang) * 1.3;
        let z = rad * sin(ang);
        let height = 0.9 + 0.7 * rnd(i, 3u);
        let lean = 0.25 * (rnd(i, 4u) - 0.5);
        let stiffness = 0.6 + 1.0 * rnd(i, 5u);
        let phase = 6.2831853 * rnd(i, 6u);
        let reed = emit Reed {
            xf: rigid[Reed, World](Vec[World](x, 0, z)),
            height: height, lean: lean, stiffness: stiffness, phase: phase,
        };
        let at = attachment(reed, |r| r.xf);
        let bnd = 0.6 * height;
        let bx = x;
        let bounds = box(Pt[Reed](-bnd, 0, -0.1), Pt[Reed](bnd, height + 0.2, 0.1));
        add(w, field(bounds, |p| stalk(p, height, lean, stiffness, phase, bx)), reed_ink, at);
        add(w, field(bounds, |p| seed_head(p, height, lean, stiffness, phase, bx)), head_ink, at);
    }
    add_sun(w, normalize(Vec[World](-2, 4, 1.5)), rgb(3, 3, 3));
    environment(w, |d| select(paper(), mix(paper(), ink_b(), 0.08), d.y > 0.6 && fract(d.x * 9.0) < 0.1));
    let eye = Pt[World](0, 1.0, -5.0);
    let target = Pt[World](0, 0.7, 0);
    let up = Dir[World](0, 1, 0);
    return frame(w, look_at(eye, target, up, 0.5rad), main);
}

pub fn main(w: World, c: Camera) -> Schedule {
    let s = plan(w, c);
    let n = clear_counts(s, w);
    let v = record(s, w, c, cull(s, w, c, n), vis64());
    let hdr = deferred(s, w, v, classify(s, v, n), sun_and_sky);
    return output(s, hdr, aces);
}
```

## Task 3

```
import std::render::*;

space Body, Neck, Head;

const NECK_LIMIT = 1.1rad;
const HOP_TIME = 0.42s;
const HOP_HEIGHT = 0.16m;
const TURN_RATE = 1.1;          // rad per second while airborne
const GAZE_CLEAR = 0.05;        // rad: hop starts when the neck is this far past its limit

kind HeronBody {
    row { pos: Pt[World], yaw: f32, hop_t: Sec, }
}
kind HeronHead {
    row {
        parent: Row[HeronBody],
        eye: Pt[World], hips: Pt[World],
        rel: f32,            // bearing of the fish relative to the trunk heading
        head_yaw: f32,       // head yaw relative to the trunk, inside the limit
        pitch: f32,
        missed: f32,         // leftover angle beyond the limit; 0 when inside
    }
}

fn fish_pos(t: f32) -> Pt[World] {
    return Pt[World](2.2 * sin(0.45 * t), 0.05 + 0.04 * sin(1.7 * t), 1.6 + 1.2 * cos(0.45 * t) - 1.2);
}
fn wrap(a: f32) -> f32 {
    return a - 6.2831853 * floor((a + 3.1415927) / 6.2831853);
}
fn bearing_rel(pos: Pt[World], yaw: f32, t: f32) -> f32 {
    let f = fish_pos(t);
    return wrap(atan2(f.x - pos.x, f.z - pos.z) - yaw);
}
fn quat_y(a: f32) -> Quat { return quat(0.0, sin(0.5 * a), 0.0, cos(0.5 * a)); }
fn quat_x(a: f32) -> Quat { return quat(sin(0.5 * a), 0.0, 0.0, cos(0.5 * a)); }
fn body_pose(pos: Pt[World], yaw: f32) -> Xform[Body, World] {
    return rigid[Body, World](Vec[World](pos.x, pos.y, pos.z), quat_y(yaw));
}
fn root() -> Xform[World, World] { return Xform[World, World]::identity(); }

// ---------- laws ----------
pub law body_law(b: HeronBody, dt: Sec) -> HeronBody {
    var next = b;
    let t = time() / 1s;
    let rel = bearing_rel(b.pos, b.yaw, t);
    let excess = abs(rel) - NECK_LIMIT / 1rad;
    if b.hop_t > 0s {
        let h = b.hop_t + dt;
        if h >= HOP_TIME {
            next.hop_t = 0s;
            next.pos.y = 0m;
        } else {
            let u = h / HOP_TIME;
            next.hop_t = h;
            next.pos.y = HOP_HEIGHT * 4.0 * u * (1.0 - u);
            let step = min(TURN_RATE * (dt / 1s), abs(rel));
            next.yaw = b.yaw + select(-step, step, rel > 0.0);
        }
    } else if excess > GAZE_CLEAR {
        next.hop_t = dt;
    }
    return next;
}

pub law head_law(h: HeronHead, bodies: Table[HeronBody], dt: Sec) -> HeronHead {
    let b = bodies[h.parent];
    var next = h;
    let t = time() / 1s;
    let pose = body_pose(b.pos, b.yaw);
    let eye = pose * Pt[Body](0, 1.25, 0.42);

    // gaze lock first: aim at the fish in world space from the current eye
    let f = fish_pos(t);
    let rel = wrap(atan2(f.x - eye.x, f.z - eye.z) - b.yaw);
    let horiz = length(vec2(f.x - eye.x, f.z - eye.z));
    let pitch = atan2(f.y - eye.y, horiz);

    // then the neck limit
    let lim = NECK_LIMIT / 1rad;
    next.eye = eye;
    next.hips = pose * Pt[Body](0, 0.55, 0);
    next.rel = rel;
    next.head_yaw = clamp(rel, -lim, lim);
    next.pitch = clamp(pitch, -0.8, 0.8);
    next.missed = max(abs(rel) - lim, 0.0);
    return next;
}

// ---------- constraints ----------
pub constraint gaze_held(h: HeronHead) -> Verdict[Rad] {
    if !is_finite(h.rel) { return Verdict::Unknown(NonFiniteGaze); }
    if h.missed > 0.0 { return Verdict::Missed(h.missed * 1rad); }
    return Verdict::Held((NECK_LIMIT / 1rad - abs(h.rel)) * 1rad);
}

pub constraint head_above_hips(h: HeronHead) -> Verdict[Meters] {
    if !is_finite(h.eye) { return Verdict::Unknown(NonFinitePose); }
    let margin = h.eye.y - h.hips.y - 0.3m;
    if margin < 0m { return Verdict::Missed(-margin); }
    return Verdict::Held(margin);
}

// ---------- materials ----------
fn feather(h: Hit) -> Bsdf { return ggx(rgb(0.52, 0.58, 0.64), 0.6, 0.0); }
fn beak_mat(h: Hit) -> Bsdf { return ggx(rgb(0.92, 0.7, 0.2), 0.35, 0.0); }
fn water(h: Hit) -> Bsdf { return coat(ggx(rgb(0.05, 0.12, 0.14), 0.4, 0.0), 0.8, 0.05, 1.33); }
fn fish_mat(h: Hit) -> Bsdf { return ggx(rgb(0.8, 0.45, 0.25), 0.3, 0.6); }

// ---------- build ----------
pub comptime fn scene() -> Scene {
    let w = world();
    let p0 = Pt[World](0, 0, 0);
    let body = emit HeronBody { pos: p0, yaw: 0.0, hop_t: 0s };
    let head = emit HeronHead {
        parent: body,
        eye: Pt[World](0, 1.25, 0.42), hips: Pt[World](0, 0.55, 0),
        rel: 0.0, head_yaw: 0.0, pitch: 0.0, missed: 0.0,
    };

    let body_at = attachment(body, |r| body_pose(r.pos, r.yaw));
    let head_at = attachment(head, |r| rigid[Head, World](
        Vec[World](r.eye.x, r.eye.y, r.eye.z),
        quat_y(bearing_world(r)) * quat_x(-r.pitch)));
    let neck_at = attachment(body, |r| body_pose(r.pos, r.yaw) * rigid[Neck, Body](Vec[Body](0, 0.75, 0.2)));
    let fixed = attachment(body, |r| root());

    // water
    add(w, plane(Dir[World](0, 1, 0), -0.02m), water, fixed);

    // trunk
    add(w, capsule(Pt[Body](0, 0.6, -0.25), Pt[Body](0, 0.72, 0.2), 0.13m), feather, body_at);

    // two legs
    comptime for i in 0u..2u {
        let sx = select(-0.07, 0.07, i == 1u);
        add(w, capsule(Pt[Body](sx, 0.55, 0), Pt[Body](sx, 0.0, 0.02), 0.014m), feather, body_at);
    }

    // S-shaped neck, in neck space
    add(w, field(box(Pt[Neck](-0.2, -0.1, -0.3), Pt[Neck](0.2, 0.7, 0.3)),
                 |p| smin_poly(
                     smin_poly(sd_capsule(p, Pt[Neck](0, 0.0, 0.0), Pt[Neck](0, 0.2, -0.12), 0.04m),
                               sd_capsule(p, Pt[Neck](0, 0.2, -0.12), Pt[Neck](0, 0.38, 0.1), 0.035m), 0.03m),
                     sd_capsule(p, Pt[Neck](0, 0.38, 0.1), Pt[Neck](0, 0.5, 0.0), 0.03m), 0.03m)),
        feather, neck_at);

    // head and beak, on the head's forward axis (+z)
    add(w, sphere(0.05m), feather, head_at);
    add(w, capsule(Pt[Head](0, 0, 0.04), Pt[Head](0, 0, 0.34), 0.018m), beak_mat, head_at);

    // fish
    add(w, field(box(Pt[World](-4, -0.2, -4), Pt[World](4, 0.5, 4)),
                 |p| length(p - fish_pos(time() / 1s)) - 0.05m), fish_mat, fixed);

    add_sun(w, normalize(Vec[World](-3, 5, 2)), rgb(3, 3, 2.5));
    environment(w, |d| rgb(0.4, 0.5, 0.6));

    let eye = Pt[World](3.5, 1.5, -3.5);
    let target = Pt[World](0, 0.7, 0.5);
    let up = Dir[World](0, 1, 0);
    return frame(w, look_at(eye, target, up, 0.6rad), main);
}

fn bearing_world(h: HeronHead) -> f32 {
    // absolute head yaw = trunk yaw + clamped relative yaw; the trunk yaw is recovered from the fish bearing
    return atan2(fish_pos(time() / 1s).x - h.eye.x, fish_pos(time() / 1s).z - h.eye.z) - (h.rel - h.head_yaw);
}

pub fn main(w: World, c: Camera) -> Schedule {
    let s = plan(w, c);
    run_laws(s, w, [body_law, head_law]);
    check(s, w, gaze_held);
    check(s, w, head_above_hips);
    let n = clear_counts(s, w);
    let v = record(s, w, c, cull(s, w, c, n), vis64());
    let hdr = deferred(s, w, v, classify(s, v, n), sun_and_sky);
    return output(s, hdr, aces);
}
```
