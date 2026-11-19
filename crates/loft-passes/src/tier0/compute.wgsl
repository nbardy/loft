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
fn tier_word(ctx: TierContext, pc: u32) -> u32 { return tier_tape[pc + 1u]; }
fn tier_constant(ctx: TierContext, slot: u32) -> vec4f { return tier_constants[slot]; }
fn tier_row(ctx: TierContext, slot: u32) -> vec4f { return tier_rows[ctx.row + slot]; }
fn tier_input_at(ctx: TierContext, point: vec3f, slot: u32) -> vec4f { return tier_input[slot]; }
fn tier_frame_at(ctx: TierContext, slot: u32) -> vec4f { return tier_frame[slot]; }

fn tier_lut_sample(ctx: TierContext, slot: u32, u: f32) -> vec4f {
    let info = vec2u(tier_lut[slot].xy);
    let x = clamp(u, 0., 1.) * f32(info.y - 1u);
    let i = u32(x);
    return mix(tier_lut[info.x + i], tier_lut[info.x + min(i + 1u, info.y - 1u)], tier_quintic(fract(x)));
}
fn tier_tex_sample(ctx: TierContext, slot: u32, uv: vec2f) -> vec4f {
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
    return tier_eval(TierContext(row), vec3f(0.));
}
@compute @workgroup_size(32) fn loft_tier0_main(@builtin(global_invocation_id) gid: vec3u) {
    if gid.x < arrayLength(&tier_out) {
        tier_out[gid.x] = loft_tier0(gid.x);
    }
}
