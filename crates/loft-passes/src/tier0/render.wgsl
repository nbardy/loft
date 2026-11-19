fn tier_count(ctx: TierContext) -> u32 { return bitcast<u32>(rows[ctx.base]); }
fn tier_word(ctx: TierContext, pc: u32) -> u32 {
    return bitcast<u32>(rows[ctx.base + TIER_HEADER + pc]);
}
fn tier_constant(ctx: TierContext, slot: u32) -> vec4f {
    let i = ctx.base + TIER_CONST + slot * 4u;
    return vec4f(rows[i], rows[i + 1u], rows[i + 2u], rows[i + 3u]);
}
fn tier_row(ctx: TierContext, slot: u32) -> vec4f { return ctx.row[slot]; }
fn tier_input_at(ctx: TierContext, point: vec3f, slot: u32) -> vec4f { return vec4f(point, 0.0); }
fn tier_frame_at(ctx: TierContext, slot: u32) -> vec4f { return ctx.clock; }

// Neither asset slot is admitted by the authored trace lowerer until a bound
// resource with the fixed opcode's sampling contract is available.
fn tier_lut_sample(ctx: TierContext, slot: u32, u: f32) -> vec4f { return vec4f(bitcast<f32>(0x7fc00000u)); }
fn tier_tex_sample(ctx: TierContext, slot: u32, uv: vec2f) -> vec4f { return vec4f(bitcast<f32>(0x7fc00000u)); }

fn tier_operand(ctx: TierContext, word: u32) -> vec4f {
    let slot = bitcast<u32>(rows[ctx.base + word + 1u]);
    switch bitcast<u32>(rows[ctx.base + word]) {
        case 0u: { return vec4f(0.0); }
        case 1u: { return tier_constant(ctx, slot); }
        case 2u: { return tier_row(ctx, slot); }
        case 3u: { return tier_frame_at(ctx, slot); }
        default: { return vec4f(bitcast<f32>(0x7fc00000u)); }
    }
}
struct TierField { op: u32, center: vec3f, extent: vec3f }
fn tier_field(ctx: TierContext, at: u32) -> TierField {
    return TierField(bitcast<u32>(rows[ctx.base + at]),
        tier_operand(ctx, at + 1u).xyz, tier_operand(ctx, at + 3u).xyz);
}

struct TierPair { a: TierField, b: TierField, intersection: bool }
fn tier_pair(ctx: TierContext, op: u32) -> TierPair {
    return TierPair(tier_field(ctx, 2u), tier_field(ctx, 7u), op == 16u);
}

fn tier_field_distance(shape: TierField, point: vec3f) -> f32 {
    switch shape.op {
        case 42u: { return tier_sphere(point - shape.center, shape.extent.x); }
        case 43u: { return tier_box(point - shape.center, shape.extent); }
        case 7u, 8u, 9u: { return point[shape.op - 7u] - shape.extent.x; }
        default: { return bitcast<f32>(0x7fc00000u); }
    }
}

fn tier_pair_distance(pair: TierPair, point: vec3f) -> f32 {
    let a = tier_field_distance(pair.a, point);
    let b = tier_field_distance(pair.b, point);
    return select(min(a, b), max(a, b), pair.intersection);
}

fn q_trace(row: Row, point: vec3f) -> f32 {
    let base = kp.n_rows * STRIDE;
    let ctx = tier_stage(row, base);
    let mode = bitcast<u32>(rows[base + 1u]);
    switch mode {
        case 0u: { return tier_eval(ctx, point).x; }
        case 3u: { return tier_eval_compact(ctx, point).x; }
        case 15u, 16u: { return tier_pair_distance(tier_pair(ctx, mode), point); }
        case 7u, 8u, 9u, 42u, 43u: { return tier_field_distance(tier_field(ctx, 2u), point); }
        default: { return bitcast<f32>(0x7fc00000u); }
    }
}

fn loft_edit_march(row: Row, ro: vec3f, rd: vec3f, span: vec2f) -> f32 {
    let base = kp.n_rows * STRIDE;
    let ctx = tier_stage(row, base);
    let mode = bitcast<u32>(rows[base + 1u]);
    switch mode {
        case 0u: { return tier_march_tape(ctx, ro, rd, span, row.loft_lip); }
        case 3u: { return tier_march_compact(ctx, ro, rd, span, row.loft_lip); }
        case 15u, 16u: { return tier_march_pair(tier_pair(ctx, mode), ro, rd, span, row.loft_lip); }
        case 7u, 8u, 9u: { return tier_march_plane(tier_field(ctx, 2u), ro, rd, span, row.loft_lip); }
        case 42u: { return tier_march_sphere(tier_field(ctx, 2u), ro, rd, span, row.loft_lip); }
        case 43u: { return tier_march_box(tier_field(ctx, 2u), ro, rd, span, row.loft_lip); }
        default: { return bitcast<f32>(0x7fc00000u); }
    }
}

fn loft_edit_normal_ao(row: Row, p: vec3f, t: f32) -> vec4f {
    let base = kp.n_rows * STRIDE;
    let ctx = tier_stage(row, base);
    let mode = bitcast<u32>(rows[base + 1u]);
    switch mode {
        case 0u: { return tier_shade_tape(ctx, p, t); }
        case 3u: { return tier_shade_compact(ctx, p, t); }
        case 15u, 16u: { return tier_shade_pair(tier_pair(ctx, mode), p, t); }
        case 7u, 8u, 9u: { return tier_shade_plane(tier_field(ctx, 2u), p, t); }
        case 42u: { return tier_shade_sphere(tier_field(ctx, 2u), p, t); }
        case 43u: { return tier_shade_box(tier_field(ctx, 2u), p, t); }
        default: { return vec4f(bitcast<f32>(0x7fc00000u)); }
    }
}
