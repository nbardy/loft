use super::*;

pub fn shadow_kind_key(name: &str) -> u32 {
    u32::from_le_bytes(blake3::hash(name.as_bytes()).as_bytes()[..4].try_into().unwrap())
}

pub(in crate::emit) fn visibility<'a>(
    p: &Prog<'a>, types: &[Ty], luts: &[LutRef], light: &str,
) -> Res<Option<String>> {
    if light != "woodblock_pigment" || p.worlds.len() < 2 {
        return Ok(None);
    }
    let Some(pass) = p.schedule.and_then(|s| s.passes.iter().find(|pass| pass.verb.name == "find_shadow_map")) else {
        return Ok(None);
    };
    let mut em = Emitter::new(p, types);
    let mut exact = Vec::new();
    let mut keys = BTreeSet::new();
    for (i, name) in crate::kind_args(p, pass)?.into_iter().enumerate() {
        let kind = p.kinds[name];
        let Some(query) = kind.query(QueryKind::Visible) else { return Ok(None) };
        let key = shadow_kind_key(name);
        if !keys.insert(key) {
            return Err(Diag::new(Code::E520, kind.name.span, "shadow world kind key collision", "rename one caster kind"));
        }
        let mut caster = Emitter::new(p, types);
        caster.luts = luts;
        caster.row_header(kind);
        let (fns, constants) = reachable(p, &[query], &[]);
        caster.library(&fns, &constants);
        caster.func(query, "q_visible");
        let row = row_arena_source(&caster.out).replace("world_words[", "loft_shadow_rows[")
            .replace("world_luts[", "luts[");
        em.out.push_str(&namespace(&row, i));
        em.out.push('\n');
        exact.push(format!(r"
    case {key}u: {{
      let row = kind{i}_load_row(leaf.row_id);
      let xf = loft_xf_inv(kind{i}_loft_pose(row));
      let t = kind{i}_q_visible(row, loft_xf_pt(xf, ray.o + ray.t0 * ray.d), loft_xf_vec(xf, ray.d));
      return select(-1.0, ray.t0 + t, t >= 0.0);
    }}"));
    }
    if exact.is_empty() { return Ok(None); }
    em.out.push_str(crate::bvh::WGSL);
    let _ = writeln!(em.out, r"
@group(2) @binding(8) var<storage, read> loft_shadow_rows: array<f32>;
@group(2) @binding(9) var<storage, read> loft_bvh_nodes: array<LoftBvhNode>;
@group(2) @binding(10) var<storage, read> loft_bvh_leaves: array<LoftLeafRef>;
fn loft_bvh_field(leaf: LoftLeafRef, p: vec3f) -> f32 {{ return 1e6; }}
fn loft_bvh_intersect(leaf: LoftLeafRef, ray: LoftRay) -> f32 {{
  switch leaf.kind_id {{ {} default: {{ return -2.0; }} }}
}}
fn loft_shadow_world(p: vec3f, n: vec3f, sun: vec3f) -> f32 {{
  if dot(n, sun) <= 0.0 {{ return 1.0; }}
  let ray = LoftRay(p + n * 0.0004, sun, 0.0001, 1e6);
  let hit = loft_bvh_trace(ray, arrayLength(&loft_bvh_nodes), true);
  return select(0.0, 1.0, hit.t == -1.0);
}}
", exact.join("\n"));
    Ok(Some(em.out))
}
