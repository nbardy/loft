// Primary visibility for engines/coral_shrine.loft. Row offsets are in f32 words.
struct ShrineCamera {
    projection: mat4x4f,
    inverse_projection: mat4x4f,
    eye: vec4f,
    sun: vec4f,
    size: vec4u,
    flags: vec4u,
    light_projection: mat4x4f,
}
struct ShrineRows { kind: u32, base: u32, count: u32, reserved: u32 }
@group(0) @binding(0) var<uniform> shrine_camera: ShrineCamera;
@group(1) @binding(0) var<storage, read> shrine_words: array<f32>;
@group(1) @binding(1) var<storage, read> shrine_visible: array<u32>;
@group(1) @binding(2) var<uniform> shrine_rows: ShrineRows;

struct ProxyVertex {
    @builtin(position) position: vec4f,
    @location(0) world: vec3f,
    @location(1) @interpolate(flat) row: u32,
}
struct PrimaryHit { @location(0) record: vec2u, @builtin(frag_depth) depth: f32 }

fn shrine_vec(i: u32) -> vec3f {
    return vec3f(shrine_words[i], shrine_words[i + 1u], shrine_words[i + 2u]);
}

fn proxy(vertex: u32, instance: u32, stride: u32) -> ProxyVertex {
    const corners = array(1u, 3u, 7u, 1u, 7u, 5u, 0u, 6u, 2u, 0u, 4u, 6u,
        2u, 6u, 7u, 2u, 7u, 3u, 0u, 1u, 5u, 0u, 5u, 4u,
        4u, 5u, 7u, 4u, 7u, 6u, 0u, 2u, 3u, 0u, 3u, 1u);
    let row = shrine_visible[shrine_rows.base + instance];
    if row == 0xffffffffu {
        return ProxyVertex(vec4f(2, 2, 2, 1), vec3f(0), row);
    }
    let bounds = row * stride + stride - 7u;
    let corner = vec3f(vec3u(corners[vertex]) >> vec3u(0, 1, 2) & vec3u(1));
    let world = mix(shrine_vec(bounds), shrine_vec(bounds + 3u), corner);
    return ProxyVertex(shrine_camera.projection * vec4f(world, 1), world, row);
}

@vertex fn floor_proxy(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> ProxyVertex {
    return proxy(v, i, 7u);
}
@vertex fn lantern_proxy(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> ProxyVertex {
    return proxy(v, i, 20u);
}
@vertex fn branch_proxy(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> ProxyVertex {
    return proxy(v, i, 21u);
}

struct ShrineRay { origin: vec3f, dir: vec3f }
fn camera_ray(v: ProxyVertex) -> ShrineRay {
    if shrine_camera.flags.x == 0xffffffffu { discard; }
    let origin = shrine_camera.eye.xyz;
    return ShrineRay(origin, normalize(v.world - origin));
}
fn local_ray(ray: ShrineRay, row: u32) -> ShrineRay {
    let inverse = transpose(mat3x3f(shrine_vec(row), shrine_vec(row + 3u), shrine_vec(row + 6u)));
    return ShrineRay(inverse * ray.origin + -(inverse * shrine_vec(row + 9u)), inverse * ray.dir);
}
fn primary_hit(ray: ShrineRay, row: u32, t: f32) -> PrimaryHit {
    if t < 0.0 { discard; }
    let clip = shrine_camera.projection * vec4f(ray.origin + ray.dir * t, 1.0);
    let depth = clip.z / clip.w;
    return PrimaryHit(vec2u((shrine_rows.kind << 24u) | row, bitcast<u32>(t)), depth);
}

fn slab_hit(o: vec3f, d: vec3f, extents: vec3f) -> f32 {
    var enter = -1e30;
    var leave = 1e30;
    for (var axis = 0u; axis < 3u; axis++) {
        if d[axis] == 0.0 {
            if abs(o[axis]) > extents[axis] { return -1.0; }
        } else {
            let near = (-extents[axis] - o[axis]) / d[axis];
            let far = (extents[axis] - o[axis]) / d[axis];
            enter = max(enter, min(near, far));
            leave = min(leave, max(near, far));
        }
    }
    if enter > leave || leave < 0.0 { return -1.0; }
    return select(leave, enter, enter >= 0.0);
}

@fragment fn floor_hit(v: ProxyVertex) -> PrimaryHit {
    let ray = camera_ray(v);
    return primary_hit(ray, v.row,
        slab_hit(ray.origin - vec3f(0, -0.15, 0), ray.dir, vec3f(5.8, 0.15, 5.8)));
}

fn bounds_interval(ray: ShrineRay, lo: vec3f, hi: vec3f) -> vec2f {
    let inverse = 1.0 / ray.dir;
    let a = (lo - ray.origin) * inverse;
    let b = (hi - ray.origin) * inverse;
    let near = min(a, b);
    let far = max(a, b);
    return vec2f(max(max(near.x, near.y), near.z), min(min(far.x, far.y), far.z));
}
fn box_distance(p: vec3f, half_size: vec3f) -> f32 {
    let d = abs(p) - half_size;
    return length(max(d, vec3f(0))) + min(max(d.x, max(d.y, d.z)), 0.0);
}
fn lantern_fold(p: vec3f) -> vec3f {
    let q = abs(p) * 1.45 - vec3f(0.42, 0.28, 0.35);
    return vec3f(0.8525245 * q.x + 0.5226872 * q.z, q.y, 0.8525245 * q.z - 0.5226872 * q.x);
}
fn lantern_distance(p: vec3f, solid: bool) -> f32 {
    let box = box_distance(p, vec3f(0.82, 0.90, 0.82));
    if solid { return box; }
    let folded = lantern_fold(lantern_fold(lantern_fold(lantern_fold(p))));
    let carving = (box_distance(folded, vec3f(0.32, 0.45, 0.32) - vec3f(0.04)) - 0.04) / 4.420506;
    return max(max(carving, box), 0.28 - length(vec3f(p.x, 0, p.z)));
}

@fragment fn lantern_hit(v: ProxyVertex) -> PrimaryHit {
    let ray = camera_ray(v);
    let row = v.row * 20u;
    let local = local_ray(ray, row);
    let interval = bounds_interval(ray, shrine_vec(row + 13u), shrine_vec(row + 16u));
    let step_scale = 1.0 / shrine_words[row + 19u];
    let solid = shrine_words[row + 12u] != 0.0;
    var t = max(interval.x, 0.0);
    var hit = false;
    for (var step = 0u; step < 192u; step++) {
        let d = lantern_distance(local.origin + local.dir * t, solid);
        if d < 1e-4 + 2e-4 * t {
            hit = true;
            break;
        }
        t += d * step_scale;
        if t > interval.y { break; }
    }
    if !hit { discard; }
    return primary_hit(ray, v.row, t);
}

const BRANCH_AXIS = vec3f(0, 0.8660254, 0.5);
const TAPER: f32 = 0.034375;

fn in_cone(t: f32, lo: f32, hi: f32, y: f32, dy: f32) -> f32 {
    let h = y + t * dy;
    if t >= 0.0 && h >= lo && h <= hi { return t; }
    return -1.0;
}
fn cap(o: vec3f, d: vec3f, center: vec3f, radius: f32, end: f32, sign: f32) -> f32 {
    let shift = dot(center - o, d);
    let relative = o + d * shift - center;
    let b = dot(relative, d);
    let h = b * b - dot(relative, relative) + radius * radius;
    if h < 0.0 { return -1.0; }
    let root = sqrt(h);
    let near = shift - b - root;
    if near >= 0.0 && sign * (dot(relative + d * (near - shift), BRANCH_AXIS) - end) >= 0.0 {
        return near;
    }
    let far = shift - b + root;
    if far >= 0.0 && sign * (dot(relative + d * (far - shift), BRANCH_AXIS) - end) >= 0.0 {
        return far;
    }
    return -1.0;
}
fn nearest_hit(a: f32, b: f32) -> f32 {
    if a < 0.0 { return b; }
    if b < 0.0 { return a; }
    return min(a, b);
}
fn cone_and_caps(ro: vec3f, rd: vec3f, len: f32) -> f32 {
    let ra = 0.171875 * len;
    let rb = ra - TAPER * len;
    let base = vec3f(0);
    let axis = BRANCH_AXIS;
    let tip = base + axis * len;
    let shift = dot(base + axis * (len * 0.5) - ro, rd);
    let rebased = ro + rd * shift;
    let v = rebased - base;
    let y = dot(v, axis);
    let dy = dot(rd, axis);
    let radial = v - axis * y;
    let dr = rd - axis * dy;
    let taper = TAPER;
    let c2 = 1.0 - taper * taper;
    let r = ra - taper * y;
    let qa = c2 * dot(dr, dr) - taper * taper * dy * dy;
    let qb = c2 * dot(radial, dr) + taper * dy * r;
    let qc = c2 * dot(radial, radial) - r * r;
    let lo = ra * taper;
    let hi = len + rb * taper;
    let h = qb * qb - qa * qc;
    let margin = 0.0001 * (dot(v, v) + len * len);
    if qa > 0.0001 && h < -margin { return -1.0; }
    var side = -1.0;
    let oy = dot(ro - base, axis);
    if qa == 0.0 {
        if qb != 0.0 { side = in_cone(shift - qc / (2.0 * qb), lo, hi, oy, dy); }
    } else {
        if h >= 0.0 {
            let root = sqrt(h);
            let enter = in_cone(shift + (-qb - root) / qa, lo, hi, oy, dy);
            if enter >= 0.0 { return enter; }
            side = in_cone(shift + (-qb + root) / qa, lo, hi, oy, dy);
        }
    }
    let bottom = cap(ro, rd, base, ra, lo, -1.0);
    let top = cap(ro, rd, tip, rb, rb * taper, 1.0);
    return nearest_hit(side, nearest_hit(bottom, top));
}

@fragment fn branch_hit(v: ProxyVertex) -> PrimaryHit {
    if shrine_camera.flags.x == 0xffffffffu { discard; }
    let row = v.row * 21u;
    var ro = shrine_camera.eye.xyz;
    var rd = normalize(v.world - ro);
    // Keep the ray in the same arithmetic form for both view conventions.
    // The primary schedule supplies a perspective camera.
    if shrine_camera.flags.y == 1u {
        rd = shrine_camera.eye.xyz;
        ro = v.world - rd * (2.0 * length(shrine_vec(row + 17u) - shrine_vec(row + 14u)) + 1.0);
    }
    let inverse = transpose(mat3x3f(shrine_vec(row), shrine_vec(row + 3u), shrine_vec(row + 6u)));
    let offset = -(inverse * shrine_vec(row + 9u));
    let origin = inverse * ro + offset;
    let direction = inverse * rd;
    let t = cone_and_caps(origin, direction, shrine_words[row + 12u]);
    if t < 0.0 { discard; }
    let clip = shrine_camera.projection * vec4f(ro + rd * t, 1.0);
    let depth = clip.z / clip.w;
    return PrimaryHit(vec2u((shrine_rows.kind << 24u) | v.row, bitcast<u32>(t)), depth);
}
