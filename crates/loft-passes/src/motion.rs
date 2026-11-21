//! Lower consecutive row-local operations to a single fixed-step compute entry.
use super::*;
use crate::program::{Dispatch, DispatchBinding, DispatchResource, DispatchRows};
use loft_abi::AdvanceArgs;

fn unsupported(sp: Span, what: &str) -> Diag {
    Diag::new(Code::E405, sp, what, "use a row-local operation on a kind")
}

pub fn motion_module(
    p: &Prog,
    types: &[Ty],
    kind: &KindDecl,
    priors: &std::collections::BTreeMap<String, u32>,
    output: impl FnOnce() -> Res<(usize, DispatchRows)>,
) -> Res<Option<(String, Dispatch)>> {
    let mut laws = vec![];
    let mut checks = vec![];
    let mut fields = vec![];
    let mut tier = Determinism::Det;
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
                        "row-local laws take the target row and dt",
                    ));
                }
                let law = p.laws[n.as_str()];
                tier = law.tier;
                fields.extend(law.writes.iter().map(|w| w.name.clone()));
                laws.push(f);
            } else {
                if f.params.len() != 1 {
                    return Err(unsupported(
                        f.name.span,
                        "row-local checks take one target row",
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
    em.header(abi::NONCE_PLACEHOLDER);
    em.row_header(kind);
    em.out = em.out.replace(
        "@group(1) @binding(0) var<storage, read> rows:",
        "@group(2) @binding(0) var<storage, read_write> rows:",
    );
    em.motion = true;
    em.motion_tier = tier;
    if tier.is_det() {
        em.out.push_str(loft_abi::DET_WGSL);
    } else {
        // Both tiers expose tick time; the Det math prelude declares it for Det.
        em.out.push_str("var<private> loft_tick_time: f32;\n");
    }
    em.out.push_str(AdvanceArgs::WGSL);
    em.out.push_str(
        r#"
struct DispatchArgs { ticks: AdvanceArgs, first_row: u32, end_row: u32, instance_id: u32 }
@group(1) @binding(0) var<storage, read> loft_advance_args: DispatchArgs;
@group(2) @binding(1) var<storage, read_write> loft_results: array<u32>;
struct CheckValue { tag: u32, reason: u32, value: f32 }
var<private> loft_tick: u32;
"#,
    );
    let roots: Vec<_> = laws.iter().chain(&checks).copied().collect();
    let (lib, constants) = reachable(p, &roots, &[]);
    let has_call = |name| roots.iter().chain(&lib).any(|f|
        References::function(f).calls.iter().any(|c| c.name == name));
    let has_events = has_call("emit_event");
    let has_priors = has_call("sample_prior");
    if has_priors {
        em.priors = Some(priors);
        em.out.push_str(crate::plan::PRIOR_WGSL);
    }
    if has_events {
        em.out
            .push_str(&crate::fusion::event_wgsl(2, 2, "loft_tick"));
    }
    em.constants(&constants);
    for f in lib {
        em.func(f, &format!("motion_{}", fn_symbol(&f.name.name)));
    }
    for f in &laws {
        em.locals = f.params.iter().map(|p| p.name.name.clone()).collect();
        let row = wgsl_name(&f.params[0].name.name);
        writeln!(
            em.out,
            "fn law_{}(initial: Row, loft_local_dt: f32) -> Row {{ var {row} = initial;",
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
            "fn check_{}({}: Row) -> CheckValue {{",
            f.name.name,
            wgsl_name(&f.params[0].name.name)
        )
        .unwrap();
        em.block_body(&f.body, 1);
        em.out.push_str("}\n");
    }
    let hash = u32::from_le_bytes(
        blake3::hash(em.out.as_bytes()).as_bytes()[..4]
            .try_into()
            .unwrap(),
    );
    writeln!(
        em.out,
        r#"
@compute @workgroup_size(64) fn advance(@builtin(global_invocation_id) gid: vec3<u32>) {{
    let i = gid.x + loft_advance_args.first_row;
    if i >= loft_advance_args.end_row {{ return; }}
    var state = load_row(i);
    let from_tick = loft_advance_args.ticks.from_tick;
    let to_tick = loft_advance_args.ticks.to_tick;
    let dt = 1.0 / 120.0;
    for (var step = 0u; step < {}u; step += 1u) {{
        if step >= to_tick - from_tick {{ break; }}
        loft_tick = from_tick + step + 1u;
        loft_tick_time = f32(loft_tick) * dt;
"#,
        AdvanceArgs::MAX_TICKS
    )
    .unwrap();
    for f in &laws {
        writeln!(em.out, "        state = law_{}(state, dt);", f.name.name).unwrap();
    }
    em.out.push_str("    }\n");
    let mut dispatch_fields = vec![];
    let mut layout = blake3::Hasher::new();
    let mut offset = 0;
    for f in &kind.row {
        let ty = em.resolve(&f.ty);
        let width = ty.words().expect("checked row field");
        layout.update(format!("{}:{ty}:{offset}:{width}\n", f.name.name).as_bytes());
        if fields.contains(&f.name.name) {
            let field = format!("state.r_{}", f.name.name);
            let lane = |j| match ty {
                Ty::F32 => field.clone(),
                Ty::I32 | Ty::U32 | Ty::Id(_) => format!("bitcast<f32>({field})"),
                Ty::Bool => format!("select(0.0, 1.0, {field})"),
                Ty::VecN(_) | Ty::Geo(..) => format!("{field}[{j}u]"),
                Ty::Xform(..) if j < 9 => format!("{field}.r[{}u][{}u]", j / 3, j % 3),
                Ty::Xform(..) => format!("{field}.t[{}u]", j - 9),
                _ => unreachable!("checked row field"),
            };
            for j in 0..width {
                writeln!(
                    em.out,
                    "    rows[i * STRIDE + {}u] = {};",
                    offset + j,
                    lane(j)
                )
                .unwrap();
            }
            dispatch_fields.push((f.name.name.clone(), offset));
        }
        offset += width;
    }
    if !checks.is_empty() {
        em.out.push_str("    loft_tick_time = f32(to_tick) * dt;\n");
    }
    for (j, f) in checks.iter().enumerate() {
        writeln!(
            em.out,
            r#"
    let v{j} = check_{}(state);
    let dst{j} = (i * {}u + {j}u) * 8u;
    let finite{j} = (bitcast<u32>(v{j}.value) & 0x7f800000u) != 0x7f800000u;
    loft_results[dst{j}] = {j}u;
    loft_results[dst{j} + 1u] = loft_advance_args.instance_id;
    loft_results[dst{j} + 2u] = i;
    loft_results[dst{j} + 3u] = to_tick;
    loft_results[dst{j} + 4u] = select(3u, v{j}.tag, finite{j});
    loft_results[dst{j} + 5u] = select(2u, v{j}.reason, finite{j});
    loft_results[dst{j} + 6u] = bitcast<u32>(v{j}.value);
    loft_results[dst{j} + 7u] = {hash}u;"#,
            f.name.name,
            checks.len()
        )
        .unwrap();
    }
    em.out.push_str("}\n");
    let mut bindings = vec![
        (1, 0, DispatchResource::Arguments),
        (2, 0, DispatchResource::Rows),
    ];
    if !checks.is_empty() {
        bindings.push((2, 1, DispatchResource::Results));
    }
    if has_priors {
        bindings.push((1, 3, DispatchResource::Luts));
    }
    if has_events {
        bindings.push((2, 2, DispatchResource::Outbox));
    }
    let wgsl = if tier.is_det() { motion_round::lower(&em.out, kind.name.span)? } else { em.out };
    Ok(Some((
        wgsl,
        Dispatch {
            tier,
            module,
            entry: AdvanceArgs::ENTRY.into(),
            workgroup_x: 64,
            input,
            bindings: bindings
                .into_iter()
                .map(|(group, slot, resource)| DispatchBinding {
                    group,
                    slot,
                    resource,
                })
                .collect(),
            fields: dispatch_fields,
            layout_hash: *layout.finalize().as_bytes(),
            checks: checks.iter().map(|c| c.name.name.clone()).collect(),
        },
    )))
}
