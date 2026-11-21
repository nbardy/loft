// Keep raw entry names letter-final. Naga rewrites names that end in digits.
// Periodic transfer coordinates are unsigned sixteenths of a cell.
// The solver's tank boundaries are applied after normalizing the deposit.
const CAPACITY: u32 = 65536u;
const CELLS: u32 = 32768u;
const SCALE: f32 = 4096.0;
const DT: f32 = 1.0 / 120.0;
const VELOCITY_UNIT: f32 = 16.0;
const LO: vec3f = vec3f(1.25);
const HI: vec3f = vec3f(29.75, 30.0, 29.75);

struct ParticleState { position: vec4f, velocity: vec4f }
struct Node { before: vec4f, after: vec4f }
struct AtomicCell { mass: atomic<i32>, x: atomic<i32>, y: atomic<i32>, z: atomic<i32> }
struct Range { first: u32, end: u32 }

@group(1) @binding(0) var<storage, read> Control: array<u32>;
@group(1) @binding(3) var<storage, read> Ranges: array<Range>;
@group(1) @binding(4) var<storage, read> KeysA: array<vec2u>;

@group(2) @binding(0) var<storage, read_write> State: array<ParticleState>;
@group(2) @binding(1) var<storage, read_write> Packed: array<vec4u>;
@group(2) @binding(2) var<storage, read_write> Status: array<u32>;
@group(2) @binding(3) var<storage, read_write> AtomicGrid: array<AtomicCell>;
@group(2) @binding(4) var<storage, read_write> FloatGrid: array<vec4f>;
@group(2) @binding(5) var<storage, read_write> Grid: array<vec4i>;
@group(2) @binding(6) var<storage, read_write> PublishedFloat: array<vec4f>;
@group(2) @binding(7) var<storage, read_write> Nodes: array<Node>;

fn cell_index(c: vec3u) -> u32 {
    let p = c & vec3u(31u);
    return p.x + 32u * (p.y + 32u * p.z);
}

fn cell_coords(i: u32) -> vec3u {
    return vec3u(i, i >> 5u, i >> 10u) & vec3u(31u);
}

fn decode_velocity(bits: u32) -> vec3i {
    return vec3i(vec3u(bits, bits >> 8u, bits >> 16u) & vec3u(127u)) - vec3i(63);
}

fn linear_offset(tap: u32) -> vec3u {
    return vec3u(tap, tap >> 1u, tap >> 2u) & vec3u(1u);
}

fn linear_weight(q: vec3u, off: vec3u) -> u32 {
    let f = q & vec3u(15u);
    let w = select(vec3u(16u) - f, f, off != vec3u(0u));
    return w.x * w.y * w.z;
}

fn quadratic_offset(tap: u32) -> vec3u {
    return vec3u(tap % 3u, (tap / 3u) % 3u, tap / 9u);
}

fn quadratic_start(q: vec3u) -> vec3u {
    return ((q + vec3u(8u)) >> vec3u(4u)) + vec3u(31u);
}

fn quadratic_weight(q: vec3u, off: vec3u) -> u32 {
    let f = (q + vec3u(8u)) & vec3u(15u);
    let d = vec3u(16u) - f;
    let left = d * d;
    let right = f * f;
    let center = vec3u(512u) - left - right;
    let w = select(select(left, center, off == vec3u(1u)), right, off == vec3u(2u));
    return w.x * w.y * w.z;
}

fn fixed_term(w: u32, divisor: u32, v: vec3i) -> vec4i {
    // Round the mass and each signed momentum independently from the full
    // numerator. Quadratic products require u32 before the signed conversion.
    let den = divisor * 63u;
    let mag = (vec3u(abs(v)) * w + vec3u(den / 2u)) / den;
    return vec4i(i32((w + divisor / 2u) / divisor), vec3i(mag) * sign(v));
}

fn clear_cell(i: u32) {
    atomicStore(&AtomicGrid[i].mass, 0);
    atomicStore(&AtomicGrid[i].x, 0);
    atomicStore(&AtomicGrid[i].y, 0);
    atomicStore(&AtomicGrid[i].z, 0);
}

fn add_cell(i: u32, p: vec4i) {
    atomicAdd(&AtomicGrid[i].mass, p.x);
    atomicAdd(&AtomicGrid[i].x, p.y);
    atomicAdd(&AtomicGrid[i].y, p.z);
    atomicAdd(&AtomicGrid[i].z, p.w);
}

@compute @workgroup_size(64)
fn pack_state(@builtin(global_invocation_id) gid: vec3u) {
    let i = gid.x;
    if Control[0] > CAPACITY || i >= Control[0] { return; }
    let p = State[i];
    let q = vec3u(floor(p.position.xyz * 16.0 + 0.5));
    let v = vec3u(floor(clamp(p.velocity.xyz / VELOCITY_UNIT, vec3f(-1.0), vec3f(1.0)) * 63.0 + 63.5));
    Packed[i] = vec4u(q, v.x | (v.y << 8u) | (v.z << 16u));
}

@compute @workgroup_size(64)
fn clear_fixed(@builtin(global_invocation_id) gid: vec3u) {
    clear_cell(gid.x);
    if gid.x == 0u { Status[0] = u32(Control[0] > CAPACITY); }
}

@compute @workgroup_size(64)
fn clear_float(@builtin(global_invocation_id) gid: vec3u) {
    FloatGrid[gid.x] = vec4f(0.0);
}

@compute @workgroup_size(1)
fn check_capacity() {
    Status[0] = u32(Control[0] > CAPACITY);
}

@compute @workgroup_size(64)
fn scatter_linear(@builtin(global_invocation_id) gid: vec3u) {
    if Status[0] != 0u || gid.x >= Control[0] { return; }
    let p = Packed[gid.x];
    let base = p.xyz >> vec3u(4u);
    let v = decode_velocity(p.w);
    for (var t = 0u; t < 8u; t++) {
        let off = linear_offset(t);
        add_cell(cell_index(base + off), fixed_term(linear_weight(p.xyz, off), 1u, v));
    }
}

@compute @workgroup_size(64)
fn scatter_quadratic(@builtin(global_invocation_id) gid: vec3u) {
    if Status[0] != 0u || gid.x >= Control[0] { return; }
    let p = Packed[gid.x];
    let start = quadratic_start(p.xyz);
    let v = decode_velocity(p.w);
    for (var t = 0u; t < 27u; t++) {
        let off = quadratic_offset(t);
        add_cell(cell_index(start + off), fixed_term(quadratic_weight(p.xyz, off), 32768u, v));
    }
}

@compute @workgroup_size(64)
fn publish_fixed(@builtin(global_invocation_id) gid: vec3u) {
    if Status[0] != 0u { return; }
    let i = gid.x;
    Grid[i] = vec4i(atomicLoad(&AtomicGrid[i].mass), atomicLoad(&AtomicGrid[i].x),
                   atomicLoad(&AtomicGrid[i].y), atomicLoad(&AtomicGrid[i].z));
}

@compute @workgroup_size(64)
fn publish_float(@builtin(global_invocation_id) gid: vec3u) {
    if Status[0] != 0u { return; }
    let i = gid.x;
    PublishedFloat[i] = (FloatGrid[i] + FloatGrid[i + CELLS])
                       + (FloatGrid[i + 2u * CELLS] + FloatGrid[i + 3u * CELLS]);
}

@compute @workgroup_size(64)
fn gather_sorted(@builtin(global_invocation_id) gid: vec3u) {
    if Status[0] != 0u { return; }
    let c = cell_coords(gid.x);
    var sum = vec4i(0);
    // Fixed direction order, then upload ID order inside each base-cell range.
    for (var t = 0u; t < 8u; t++) {
        let off = linear_offset(t);
        let r = Ranges[cell_index(c + vec3u(32u) - off)];
        for (var i = r.first; i < r.end; i++) {
            let p = Packed[KeysA[i].y];
            sum += fixed_term(linear_weight(p.xyz, off), 1u, decode_velocity(p.w));
        }
    }
    Grid[gid.x] = sum;
}

fn solve(i: u32, deposit: vec4f) {
    let mass = deposit.x;
    var old = vec3f(0.0);
    var next = old;
    if mass > 0.0 {
        old = deposit.yzw * (VELOCITY_UNIT / mass);
        next = old + vec3f(0.0, -12.0 * DT, 0.0);
        let c = vec3f(cell_coords(i));
        next = select(next, max(next, vec3f(0.0)), c <= LO + 0.75);
        next = select(next, min(next, vec3f(0.0)), c >= HI - 0.75);
    }
    Nodes[i] = Node(vec4f(old, mass), vec4f(next, mass));
}

@compute @workgroup_size(64)
fn update_fixed(@builtin(global_invocation_id) gid: vec3u) {
    if Status[0] == 0u { solve(gid.x, vec4f(Grid[gid.x]) / SCALE); }
}

@compute @workgroup_size(64)
fn update_float(@builtin(global_invocation_id) gid: vec3u) {
    if Status[0] == 0u { solve(gid.x, PublishedFloat[gid.x]); }
}

fn integrate(i: u32, pic: vec3f, delta: vec3f) {
    let p = State[i];
    let v = clamp(mix(p.velocity.xyz + delta, pic, 0.08),
                  vec3f(-VELOCITY_UNIT), vec3f(VELOCITY_UNIT));
    let moved = p.position.xyz + v * DT;
    let hit = select(moved > HI, vec3(true), moved < LO);
    State[i] = ParticleState(vec4f(clamp(moved, LO, HI), 0.0),
                             vec4f(select(v, -0.15 * v, hit), 0.0));
}

@compute @workgroup_size(64)
fn g2p_linear(@builtin(global_invocation_id) gid: vec3u) {
    let i = gid.x;
    if Status[0] != 0u || i >= Control[0] { return; }
    let q = Packed[i].xyz;
    let base = q >> vec3u(4u);
    var pic = vec3f(0.0);
    var delta = vec3f(0.0);
    for (var t = 0u; t < 8u; t++) {
        let off = linear_offset(t);
        let node = Nodes[cell_index(base + off)];
        let w = f32(linear_weight(q, off)) / SCALE;
        pic += w * node.after.xyz;
        delta += w * (node.after.xyz - node.before.xyz);
    }
    integrate(i, pic, delta);
}

@compute @workgroup_size(64)
fn g2p_quadratic(@builtin(global_invocation_id) gid: vec3u) {
    let i = gid.x;
    if Status[0] != 0u || i >= Control[0] { return; }
    let q = Packed[i].xyz;
    let start = quadratic_start(q);
    var pic = vec3f(0.0);
    var delta = vec3f(0.0);
    for (var t = 0u; t < 27u; t++) {
        let off = quadratic_offset(t);
        let node = Nodes[cell_index(start + off)];
        let w = f32(quadratic_weight(q, off)) / 134217728.0;
        pic += w * node.after.xyz;
        delta += w * (node.after.xyz - node.before.xyz);
    }
    integrate(i, pic, delta);
}
