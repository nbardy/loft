use super::*;

/// Split lobe lets that depend only on the material, normal and outgoing ray.
/// The first mutable or control-flow statement ends the portion we can move.
pub(super) fn invariant_lets<'a>(p: &Prog<'a>, lobe: &'a FnDecl) -> Vec<usize> {
    let (functions, _) = reachable(p, &[lobe], &[]);
    if lobe.raw.is_some() || functions.iter().any(|f| f.raw.is_some()) {
        return Vec::new();
    }
    let Some(incident) = lobe.params.last() else { return Vec::new() };
    let mut varying = BTreeSet::from([incident.name.name.as_str()]);
    let mut invariant = Vec::new();
    for (index, statement) in lobe.body.stmts.iter().enumerate() {
        let Stmt::Let { mutable: false, name, value, .. } = statement else { break };
        let mut refs = References::default();
        refs.expr(value, &[]);
        if refs.names.iter().any(|(name, _)| varying.contains(name)) {
            varying.insert(&name.name);
        } else {
            invariant.push(index);
        }
    }
    invariant
}

pub(super) fn emit_prepared(
    em: &mut Emitter<'_, '_>, lobes: &[(&str, usize)], invariants: &[Vec<usize>], eval_body: &str,
) {
    em.out.push_str("struct LoftPreparedBsdf {\n");
    for (i, (name, _)) in lobes.iter().enumerate() {
        let f = em.p.fns[name];
        for &j in &invariants[i] {
            let Stmt::Let { value, .. } = &f.body.stmts[j] else { unreachable!() };
            let ty = wty(em.ty_of(value));
            let _ = writeln!(em.out, "  l{i}_{j}: {ty},");
        }
    }
    em.out.push_str("}\nfn loft_prepare_bsdf(b: Bsdf, n: vec3f, v: vec3f) -> LoftPreparedBsdf {\n  var c: LoftPreparedBsdf;\n");
    for (i, (name, arity)) in lobes.iter().enumerate().filter(|(i, _)| !invariants[*i].is_empty()) {
        let f = em.p.fns[name];
        em.out.push_str("  {\n");
        em.locals.clear();
        for (arg, field) in f.params.iter().zip((0..*arity).map(|j| format!("b.l{i}_{j}")).chain(["n".into(), "v".into()])) {
            let local = wgsl_name(&arg.name.name);
            let _ = writeln!(em.out, "    let {local} = {field};");
            em.locals.push(arg.name.name.clone());
        }
        for &j in &invariants[i] {
            em.stmt(&f.body.stmts[j], 2);
            let Stmt::Let { name, .. } = &f.body.stmts[j] else { unreachable!() };
            let local = wgsl_name(&name.name);
            let _ = writeln!(em.out, "    c.l{i}_{j} = {local};");
        }
        em.out.push_str("  }\n");
    }
    em.out.push_str("  return c;\n}\n");
    for (i, (name, _)) in lobes.iter().enumerate().filter(|(i, _)| !invariants[*i].is_empty()) {
        let f = em.p.fns[name];
        let params = f.params.iter().map(|p| {
            format!("{}: {}", wgsl_name(&p.name.name), wty(&em.resolve(p.ty.as_ref().expect("typed lobe"))))
        }).collect::<Vec<_>>();
        let _ = writeln!(em.out, "fn loft_prepared_lobe_{i}({}, prepared: LoftPreparedBsdf) -> vec4f {{", params.join(", "));
        em.locals = f.params.iter().map(|p| p.name.name.clone()).collect();
        em.maps.clear();
        for (j, statement) in f.body.stmts.iter().enumerate() {
            if invariants[i].contains(&j) {
                let Stmt::Let { name, .. } = statement else { unreachable!() };
                let local = wgsl_name(&name.name);
                let _ = writeln!(em.out, "  let {local} = prepared.l{i}_{j};");
                em.locals.push(name.name.clone());
            } else {
                em.stmt(statement, 1);
            }
        }
        em.out.push_str("}\n");
    }
    let _ = writeln!(em.out, "fn loft_eval_prepared(b: Bsdf, n: vec3f, v: vec3f, l: vec3f, prepared: LoftPreparedBsdf) -> vec3f {{\n{eval_body}}}");
}
