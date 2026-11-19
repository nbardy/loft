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
fn tier_sphere_operands(ctx: TierContext) -> vec4f {
    return vec4f(tier_operand(ctx, 2u).xyz, tier_operand(ctx, 4u).x);
}

fn q_trace(row: Row, point: vec3f) -> f32 {
    let base = kp.n_rows * STRIDE;
    let ctx = tier_stage(row, base);
    // The mode is shared by the entire dirty kind.
    if bitcast<u32>(rows[base + 1u]) == 42u {
        let sphere = tier_sphere_operands(ctx);
        return tier_sphere(point - sphere.xyz, sphere.w);
    }
    return tier_eval(ctx, point).x;
}
fn loft_edit_march(row: Row, ro: vec3f, rd: vec3f, span: vec2f) -> f32 {
    let base = kp.n_rows * STRIDE;
    let ctx = tier_stage(row, base);
    if bitcast<u32>(rows[base + 1u]) == 42u {
        return tier_march_sphere(tier_sphere_operands(ctx), ro, rd, span, row.loft_lip);
    }
    return tier_march_tape(ctx, ro, rd, span, row.loft_lip);
}
fn loft_edit_normal_ao(row: Row, p: vec3f, t: f32) -> vec4f {
    let base = kp.n_rows * STRIDE;
    let ctx = tier_stage(row, base);
    if bitcast<u32>(rows[base + 1u]) == 42u {
        return tier_shade_sphere(tier_sphere_operands(ctx), p, t);
    }
    return tier_shade_tape(ctx, p, t);
}
