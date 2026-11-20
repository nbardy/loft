// One fixed entry per dirty kind, with a workgroup-uniform tape/PC.
@group(0) @binding(0) var<uniform> tier_frame: array<vec4f, 16>;
@group(0) @binding(1) var<storage, read> tier_input: array<vec4f>;
@group(1) @binding(0) var<storage, read> tier_tape: array<u32>;
@group(1) @binding(1) var<storage, read> tier_constants: array<vec4f>;
@group(1) @binding(2) var<storage, read> tier_rows: array<vec4f>;
@group(1) @binding(3) var<storage, read> tier_lut: array<vec4f>;
@group(2) @binding(0) var<storage, read_write> tier_out: array<vec4f>;
@group(3) @binding(0) var tier_texture: texture_2d_array<f32>;

struct TierContext { row: u32 }
fn tier_count(ctx: TierContext) -> u32 { return tier_tape[0]; }
var<workgroup> tier_decoded_words: array<vec2u, 2048>;
fn tier_decoded(ctx: TierContext, pc: u32) -> vec2u { return tier_decoded_words[pc]; }
fn tier_constant(ctx: TierContext, slot: u32) -> vec4f { return tier_constants[slot]; }
fn tier_row(ctx: TierContext, slot: u32) -> vec4f { return tier_rows[ctx.row + slot]; }
fn tier_input_at(ctx: TierContext, point: vec3f, slot: u32) -> vec4f { return tier_input[slot]; }
fn tier_frame_at(ctx: TierContext, slot: u32) -> vec4f { return tier_frame[slot]; }

fn tier_lut_sample(ctx: TierContext, slot: u32, u: f32) -> vec4f {
    let n = arrayLength(&tier_lut);
    if slot >= n { return vec4f(bitcast<f32>(0x7fc00000u)); }
    let info = vec2u(tier_lut[slot].xy);
    if info.x >= n || info.y == 0u || info.y > n - info.x {
        return vec4f(bitcast<f32>(0x7fc00000u));
    }
    let x = clamp(u, 0., 1.) * f32(info.y - 1u);
    let i = u32(x);
    return mix(tier_lut[info.x + i], tier_lut[info.x + min(i + 1u, info.y - 1u)], tier_quintic(fract(x)));
}
fn tier_tex_sample(ctx: TierContext, slot: u32, uv: vec2f) -> vec4f {
    if slot >= textureNumLayers(tier_texture) { return vec4f(bitcast<f32>(0x7fc00000u)); }
    let dims = vec2i(textureDimensions(tier_texture));
    let p = clamp(uv, vec2f(0.), vec2f(1.)) * vec2f(dims) - 0.5;
    let i = vec2i(floor(p));
    let lo = clamp(i, vec2i(0), dims - 1);
    let hi = clamp(i + 1, vec2i(0), dims - 1);
    let f = fract(p);
    let a = textureLoad(tier_texture, lo, i32(slot), 0);
    let b = textureLoad(tier_texture, vec2i(hi.x, lo.y), i32(slot), 0);
    let c = textureLoad(tier_texture, vec2i(lo.x, hi.y), i32(slot), 0);
    let d = textureLoad(tier_texture, hi, i32(slot), 0);
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}
fn loft_tier0(row: u32) -> vec4f {
    var bank: array<vec4f, 32>;
    return tier_run(TierContext(row), vec3f(0.), &bank, 0u, tier_count(TierContext(row)));
}
@compute @workgroup_size(32) fn loft_tier0_main(@builtin(global_invocation_id) gid: vec3u, @builtin(local_invocation_index) lane: u32) {
    for (var pc = lane; pc < tier_tape[0]; pc += 32u) {
        let ins = tier_tape[pc + 1u];
        let imm = ins & 2047u;
        tier_decoded_words[pc] = vec2u((imm << 8u) | (ins >> 26u), pack4xU8(vec4u(
            (ins >> 21u) & 31u, (ins >> 16u) & 31u, (ins >> 11u) & 31u, imm & 31u)));
    }
    workgroupBarrier();
    if gid.x < arrayLength(&tier_out) {
        tier_out[gid.x] = loft_tier0(gid.x);
    }
}
