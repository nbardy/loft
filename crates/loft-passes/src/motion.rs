//! Lower the row-local subset needed for a Phase 1.5 bakeoff snapshot.
//! Phase 3 persistent state, solver tables and checkpoints use a separate schedule ABI.
use super::*;
use loft_ir::{Span, program::{Dispatch, DispatchRows}};

fn unsupported(sp: Span, what: &str) -> Diag {
    Diag::new(
        Code::E405,
        sp,
        what,
        "use a row-local law with scalar writes for this phase",
    )
}
pub fn motion_module(
    p: &Prog,
    types: &[Ty],
    kind: &KindDecl,
    output: impl FnOnce() -> Res<(usize, DispatchRows)>,
) -> Res<Option<(String, Dispatch)>> {
    let mut laws = vec![];
    let mut checks = vec![];
    let mut fields = vec![];
    for pass in &p.schedule.expect("checked schedule").passes {
        let is_law = match pass.verb.name.as_str() {
            "run_laws" => true,
            "check" => false,
            _ => continue,
        };
        for a in &pass.args {
            let ExprKind::Name(n) = &a.value.kind else {
                return Err(unsupported(
                    a.value.span,
                    "expected a scheduled law or check name",
                ));
            };
            let f = if is_law {
                p.laws.get(n.as_str()).map(|l| &l.f)
            } else {
                p.checks.get(n.as_str()).copied()
            }
            .ok_or_else(|| {
                Diag::new(
                    Code::E011,
                    a.value.span,
                    "unknown scheduled law or check",
                    "declare the named function",
                )
            })?;
            let Some(TyExpr::Name(target)) = &f.params[0].ty else {
                return Err(unsupported(f.name.span, "motion target requires a kind"));
            };
            if target.name != kind.name.name {
                continue;
            }
            if is_law {
                if !checks.is_empty() {
                    return Err(Diag::new(
                        Code::E505,
                        a.value.span,
                        "law scheduled after its checks",
                        "schedule the row laws first and the checks last",
                    ));
                }
                if f.params.len() != 2 || f.params[1].name.name != "dt" {
                    return Err(unsupported(
                        f.name.span,
                        "snapshot laws take the target row and dt",
                    ));
                }
                for w in &p.laws[n.as_str()].writes {
                    let field = kind
                        .row
                        .iter()
                        .find(|rf| rf.name.name == w.name)
                        .expect("checked ownership");
                    if !matches!(&field.ty,TyExpr::Name(t) if t.name=="f32") {
                        return Err(unsupported(
                            w.span,
                            "snapshot requires a scalar float column",
                        ));
                    }
                    fields.push(w.name.clone());
                }
                laws.push(f);
            } else {
                if f.params.len() != 1 {
                    return Err(unsupported(
                        f.name.span,
                        "snapshot checks take one target row",
                    ));
                }
                checks.push(f);
            }
        }
    }
    if laws.is_empty() && checks.is_empty() {
        return Ok(None);
    }
    let (module, input) = output()?;
    let mut em = Emitter::new(p, types);
    em.motion = true;
    em.out.push_str(include_str!("det.wgsl"));
    let roots: Vec<_> = laws.iter().chain(&checks).copied().collect();
    let (lib, constants) = reachable(p, &roots, &[]);
    for n in constants {
        if p.param_slot(&n).is_some() { continue; }
        let c = p.consts[n.as_str()];
        let value = em.expr(&c.value);
        writeln!(
            em.out,
            "const motion_const_{}: {} = {};",
            wgsl_name(&n),
            wty(em.ty_of(&c.value)),
            value
        )
        .unwrap();
    }
    for f in lib {
        em.func(f, &format!("motion_{}", fn_symbol(&f.name.name)));
    }
    for f in &laws {
        em.locals = f.params.iter().map(|p| p.name.name.clone()).collect();
        let row = wgsl_name(&f.params[0].name.name);
        writeln!(
            em.out,
            "fn law_{}(initial: Row, dt: f32) -> Row {{ var {row} = initial;",
            f.name.name
        )
        .unwrap();
        em.block_body(&f.body, 1);
        writeln!(em.out, "return {row}; }}").unwrap();
    }
    for f in &checks {
        em.locals = f.params.iter().map(|p| p.name.name.clone()).collect();
        writeln!(
            em.out,
            "fn check_{}({}: Row) -> vec2<f32> {{",
            f.name.name,
            wgsl_name(&f.params[0].name.name)
        )
        .unwrap();
        em.block_body(&f.body, 1);
        em.out.push_str("}\n");
    }
    em.out.push_str("@compute @workgroup_size(64) fn advance(@builtin(global_invocation_id) gid: vec3<u32>) {\n let i=gid.x; if i >= arrayLength(&rows)/STRIDE { return; } var state=load_row(i);\n let last=bitcast<u32>(twin_in[0].x); let dt=1.0/120.0;\n for(var tick=1u; tick<=last; tick++){ loft_tick_time=f32(tick)*dt;\n");
    for f in &laws {
        writeln!(em.out, "state=law_{}(state,dt);", f.name.name).unwrap();
    }
    em.out.push_str("}\n");
    let width = fields.len() + checks.len();
    for (i, n) in fields.iter().enumerate() {
        writeln!(
            em.out,
            "twin_out[i*{width}u+{i}u]=vec4<f32>(state.r_{n},0.,0.,0.);"
        )
        .unwrap();
    }
    for (j, f) in checks.iter().enumerate() {
        let i = j + fields.len();
        writeln!(
            em.out,
            "twin_out[i*{width}u+{i}u]=vec4<f32>(check_{}(state),0.,0.);",
            f.name.name
        )
        .unwrap();
    }
    em.out.push_str("}\n");
    Ok(Some((
        em.out,
        Dispatch {
            module,
            entry: "advance".into(),
            workgroup_x: 64,
            input,
            fields,
            checks: checks.iter().map(|c| c.name.name.clone()).collect(),
        },
    )))
}
