// Stable LSD radix sort of a 15-bit base-cell key. Each 128-lane tile
// publishes its histogram, then reads its exclusive per-digit tile offset.
const TILE: u32 = 128u;
const TILES: u32 = 512u;
const DIGITS: u32 = 256u;
struct Range { first: u32, end: u32 }
@group(1) @binding(0) var<storage, read> Control: array<u32>;
@group(1) @binding(1) var<storage, read> Status: array<u32>;
@group(1) @binding(2) var<storage, read> Packed: array<vec4u>;
@group(2) @binding(0) var<storage, read_write> KeysA: array<vec2u>;
@group(2) @binding(1) var<storage, read_write> KeysB: array<vec2u>;
@group(2) @binding(2) var<storage, read_write> Histogram: array<u32>;
@group(2) @binding(3) var<storage, read_write> Totals: array<u32>;
@group(2) @binding(4) var<storage, read_write> Ranges: array<Range>;

var<workgroup> bins: array<atomic<u32>, 256>;
var<workgroup> tile_digits: array<u32, 128>;

@compute @workgroup_size(128)
fn make_keys(@builtin(global_invocation_id) gid: vec3u) {
    let i = gid.x;
    var key = 0xffffffffu;
    if Status[0] == 0u && i < Control[0] {
        let c = (Packed[i].xyz >> vec3u(4u)) & vec3u(31u);
        key = c.x + 32u * (c.y + 32u * c.z);
    }
    KeysA[i] = vec2u(key, i);
}

fn count_digit(key: u32, shift: u32, lane: u32, tile: u32) {
    let digit = (key >> shift) & 255u;
    atomicAdd(&bins[digit], 1u);
    workgroupBarrier();
    let offset = tile * DIGITS + lane;
    Histogram[offset] = atomicLoad(&bins[lane]);
    Histogram[offset + TILE] = atomicLoad(&bins[lane + TILE]);
}

@compute @workgroup_size(128)
fn count_low(@builtin(local_invocation_index) lane: u32, @builtin(workgroup_id) wg: vec3u) {
    count_digit(KeysA[wg.x * TILE + lane].x, 0u, lane, wg.x);
}

@compute @workgroup_size(128)
fn count_high(@builtin(local_invocation_index) lane: u32, @builtin(workgroup_id) wg: vec3u) {
    count_digit(KeysB[wg.x * TILE + lane].x, 8u, lane, wg.x);
}

@compute @workgroup_size(64)
fn scan_tiles(@builtin(global_invocation_id) gid: vec3u) {
    let digit = gid.x;
    var sum = 0u;
    for (var t = 0u; t < TILES; t++) {
        let index = t * DIGITS + digit;
        let count = Histogram[index];
        Histogram[index] = sum;
        sum += count;
    }
    Totals[digit] = sum;
}

@compute @workgroup_size(1)
fn scan_digits() {
    var sum = 0u;
    for (var d = 0u; d < DIGITS; d++) {
        let count = Totals[d];
        Totals[d] = sum;
        sum += count;
    }
}

fn destination(key: u32, shift: u32, lane: u32, tile: u32) -> u32 {
    let digit = (key >> shift) & 255u;
    tile_digits[lane] = digit;
    workgroupBarrier();
    var rank = 0u;
    for (var j = 0u; j < lane; j++) {
        rank += u32(digit == tile_digits[j]);
    }
    return Totals[digit] + Histogram[tile * DIGITS + digit] + rank;
}

@compute @workgroup_size(128)
fn scatter_low(@builtin(local_invocation_index) lane: u32, @builtin(workgroup_id) wg: vec3u) {
    let pair = KeysA[wg.x * TILE + lane];
    KeysB[destination(pair.x, 0u, lane, wg.x)] = pair;
}

@compute @workgroup_size(128)
fn scatter_high(@builtin(local_invocation_index) lane: u32, @builtin(workgroup_id) wg: vec3u) {
    let pair = KeysB[wg.x * TILE + lane];
    KeysA[destination(pair.x, 8u, lane, wg.x)] = pair;
}

@compute @workgroup_size(64)
fn clear_ranges(@builtin(global_invocation_id) gid: vec3u) {
    Ranges[gid.x] = Range(0u, 0u);
}

@compute @workgroup_size(64)
fn mark_ranges(@builtin(global_invocation_id) gid: vec3u) {
    let i = gid.x;
    if Status[0] != 0u || i >= Control[0] { return; }
    let key = KeysA[i].x;
    // Two unique writers own separate scalar members. Neither performs
    // a read-modify-write of the whole range.
    if i == 0u || KeysA[i - 1u].x != key { Ranges[key].first = i; }
    if i + 1u == Control[0] || KeysA[i + 1u].x != key { Ranges[key].end = i + 1u; }
}
