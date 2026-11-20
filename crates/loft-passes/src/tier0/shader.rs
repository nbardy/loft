// Shared with the runner's native promotion test without linking the compiler.
macro_rules! tier0_eval {
    ($name:literal, $registers:literal, $body:expr) => {
        concat!(
            "fn ",
            $name,
            "(ctx: TierContext, point: vec3f) -> vec4f {\n",
            "    var reg: array<vec4f, ",
            $registers,
            ">;\n",
            $body,
        )
    };
}

// Render preview also selects this entry after checking the allocated register IDs.
#[allow(dead_code)] // The runner's include of this file only consumes VM_WGSL.
pub(super) const SMALL_EVAL: &str = tier0_eval!("tier_eval_small", 4, include_str!("eval.wgsl"));

/// Portable compute entry for a dirty kind's count-prefixed tape buffer.
pub const VM_WGSL: &str = concat!(
    include_str!("compute.wgsl"),
    include_str!("../tier0.wgsl"),
    tier0_eval!("tier_eval", 32, include_str!("eval.wgsl")),
    tier0_eval!("tier_eval_small", 4, include_str!("eval.wgsl")),
);
