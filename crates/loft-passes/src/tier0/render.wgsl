fn tier_meta(base: u32, slot: u32) -> u32 { return bitcast<u32>(rows[base + slot]); }
fn tier_decoded(ctx: TierContext, pc: u32) -> vec2u {
    let i = ctx.plan + 2u * pc;
    return vec2u(tier_meta(ctx.base, i), tier_meta(ctx.base, i + 1u));
}
fn tier_constant(ctx: TierContext, slot: u32) -> vec4f {
    let i = ctx.constant + slot * 4u;
    return vec4f(rows[i], rows[i + 1u], rows[i + 2u], rows[i + 3u]);
}
fn tier_row(ctx: TierContext, slot: u32) -> vec4f { return ctx.row[slot]; }
fn tier_input_at(ctx: TierContext, point: vec3f, slot: u32) -> vec4f { return vec4f(point, 0.0); }
fn tier_frame_at(ctx: TierContext, slot: u32) -> vec4f { return ctx.clock; }

// Resource slots are scoped to the bound compute tape. The authored trace
// lowerer cannot emit these until its bind layout supplies matching assets.
fn tier_lut_sample(ctx: TierContext, slot: u32, u: f32) -> vec4f { return vec4f(bitcast<f32>(0x7fc00000u)); }
fn tier_tex_sample(ctx: TierContext, slot: u32, uv: vec2f) -> vec4f { return vec4f(bitcast<f32>(0x7fc00000u)); }

const TIER_TILE_WORDS: u32 = 128u;
var<workgroup> tier_cached: array<vec2u, TIER_TILE_WORDS>;
fn tier_decoded_cached(ctx: TierContext, pc: u32) -> vec2u {
    if pc < TIER_TILE_WORDS { return tier_cached[pc]; }
    return tier_decoded(ctx, pc);
}
fn tier_cache_tile(lane: u32) {
    let base = kp.n_rows * STRIDE;
    let pc_end = min(tier_meta(base, 4u), TIER_TILE_WORDS);
    let offset = tier_meta(base, 3u);
    for (var pc = lane; pc < pc_end; pc += 64u) {
        let i = offset + 2u * pc;
        tier_cached[pc] = vec2u(tier_meta(base, i), tier_meta(base, i + 1u));
    }
    // Only instructions are shared. Different lanes can hit different rows.
    workgroupBarrier();
}

fn tier_prepare(ctx: TierContext) -> array<vec4f, 32> {
    var bank: array<vec4f, 32>;
    let end = ctx.split;
    if end != 0u { _ = tier_run(ctx, vec3f(0.0), &bank, 0u, end); }
    return bank;
}
fn tier_prepare_cached(ctx: TierContext) -> array<vec4f, 32> {
    var bank: array<vec4f, 32>;
    let end = ctx.split;
    if end != 0u { _ = tier_run_cached(ctx, vec3f(0.0), &bank, 0u, end); }
    return bank;
}
fn tier_eval(ctx: TierContext, point: vec3f, staged: array<vec4f, 32>) -> vec4f {
    var bank = staged;
    return tier_run(ctx, point, &bank, ctx.split, ctx.end);
}
fn tier_eval_cached(ctx: TierContext, point: vec3f, staged: array<vec4f, 32>) -> vec4f {
    var bank = staged;
    return tier_run_cached(ctx, point, &bank, ctx.split, ctx.end);
}

fn tier_operand(ctx: TierContext, word: u32) -> vec4f {
    let slot = tier_meta(ctx.base, word + 1u);
    switch tier_meta(ctx.base, word) {
        case 0u: { return vec4f(0.0); }
        case 1u: { return tier_constant(ctx, slot); }
        case 2u: { return tier_row(ctx, slot); }
        case 3u: { return tier_frame_at(ctx, slot); }
        default: { return vec4f(bitcast<f32>(0x7fc00000u)); }
    }
}
struct TierField { op: u32, center: vec3f, extent: vec3f }
fn tier_field(ctx: TierContext, at: u32) -> TierField {
    return TierField(tier_meta(ctx.base, at),
        tier_operand(ctx, at + 1u).xyz, tier_operand(ctx, at + 3u).xyz);
}
fn tier_field_distance(shape: TierField, point: vec3f) -> f32 {
    switch shape.op {
        case 42u: { return tier_sphere(point - shape.center, shape.extent.x); }
        case 43u: { return tier_box(point - shape.center, shape.extent); }
        case 7u, 8u, 9u: { return point[shape.op - 7u] - shape.extent.x; }
        case 10u, 11u, 12u: { return -point[shape.op - 10u] - shape.extent.x; }
        case 13u, 14u, 15u: { return abs(point[shape.op - 13u]) - shape.extent.x; }
        default: { return bitcast<f32>(0x7fc00000u); }
    }
}
fn tier_join(a: f32, b: f32, intersection: bool) -> f32 {
    return select(min(a, b), max(a, b), intersection);
}
