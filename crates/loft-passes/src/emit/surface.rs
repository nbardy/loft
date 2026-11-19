use super::*;

pub(super) fn cached_grid(p: &Prog<'_>, query: &FnDecl, nu: u32, nv: u32) -> String {
    let mut refs = References::default();
    let mut local = query.params.iter().skip(1).map(|p| p.name.name.as_str()).collect();
    refs.block(&query.body, &mut local);
    if refs.names.iter().any(|(n, _)| *n == query.params[0].name.name) {
        return String::new();
    }
    let (functions, constants) = reachable(p, &[query], &[]);
    if constants.iter().any(|name| p.runtime_constant(name)) ||
        std::iter::once(query).chain(functions).any(|f| {
            let refs = References::function(f);
            f.raw.is_some() || refs.names.iter().any(|(n, _)| *n == "time")
                || refs.calls.iter().any(|call| matches!(call.name.as_str(),
                    "sun_color" | "sky_zenith" | "sky_horizon" | "has_sky"))
        }) {
        return String::new();
    }
    let cols = nu + 1;
    let rows = nv + 1;
    format!(r"
@group(2) @binding(5) var<storage, read> loft_grid: array<vec4<f32>>;
@group(2) @binding(6) var<storage, read_write> loft_grid_out: array<vec4<f32>>;

fn loft_grid_uv(i: u32) -> vec2<f32> {{
  return vec2(f32(i % {cols}u) / {nu}.0, f32(i / {cols}u) / {nv}.0);
}}

@compute @workgroup_size(64)
fn bake_surface(@builtin(global_invocation_id) id: vec3<u32>) {{
  if id.x >= {cols}u * {rows}u {{ return; }}
  let uv = loft_grid_uv(id.x);
  loft_grid_out[id.x] = vec4(q_surface(load_row(0u), uv.x, uv.y), 1.0);
}}

@vertex fn vs_indexed(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {{
  let r = visible[kp.base + ii];
  let wp = loft_xf_pt(loft_pose(load_row(r)), loft_grid[vi].xyz);
  return VsOut(frame.view_proj * vec4<f32>(wp, 1.0), loft_grid_uv(vi), r);
}}
")
}
