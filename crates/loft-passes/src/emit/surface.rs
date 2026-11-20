use super::*;

pub(super) fn indexed_grid<'a>(p: &Prog<'a>, query: &'a FnDecl, nu: u32, nv: u32) -> String {
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
    format!(r"
@vertex fn vs_grid(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {{
  let r = visible[kp.base + ii];
  if r == LOFT_EMPTY {{ return VsOut(vec4f(2.0, 2.0, 2.0, 1.0), vec2f(0.0), r); }}
  // Divide the integer coordinates by the cell counts to keep both seam endpoints.
  let uv = vec2(f32(vi % {cols}u) / {nu}.0, f32(vi / {cols}u) / {nv}.0);
  let row = load_row(r);
  let wp = loft_xf_pt(loft_pose(row), q_surface(row, uv.x, uv.y));
  return VsOut(frame.view_proj * vec4<f32>(wp, 1.0), uv, r);
}}
")
}
