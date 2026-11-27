//! Lower consecutive row-local operations to a single fixed-step compute entry.
use crate::{Prog, program::abi};
use loft_emit::emit::{Emitter, References, fn_symbol, reachable, wgsl_name};
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;
use std::fmt::Write;
use crate::program::{Dispatch, DispatchBinding, DispatchRead, DispatchResource, DispatchRows, Program};
pub use loft_abi::advance;
use loft_abi::AdvanceArgs;
pub use loft_emit::events::{event_tag, event_wgsl};
use loft_ir::{
    BindAccess, Bundle, ElemFormat, IrTy, ModuleBlob, ModuleId, ParamId, ParamMeta, PassBinding,
    PassDecl, PassId, PassRun, QueueClass, ShaderTarget, StateStoreDecl, TableDecl, TableId, TableLifetime,
};

pub const ROWS: TableId = TableId(0);
pub const RESULTS: TableId = TableId(2);

fn unsupported(sp: Span, what: &str) -> Diag {
    Diag::new(Code::E405, sp, what, "use a row-local operation on a kind")
}

pub fn motion_module(
    p: &Prog,
    types: &[Ty],
    kind: &KindDecl,
    fixed: &std::collections::BTreeMap<(String, String), String>,
    output: impl FnOnce() -> Res<(usize, DispatchRows)>,
    rows_of: impl Fn(&str, Span) -> Res<DispatchRows>,
) -> Res<Option<(String, Dispatch)>> {
    let mut laws = vec![];
    // `[S]` parameters of this kind's laws and checks, in first-use order: the tables the module binds.
    let mut reads: Vec<(String, Span)> = vec![];
    let mut tables = |f: &FnDecl, from: usize| -> Res<()> {
        for prm in &f.params[from..] {
            let Some(TyExpr::Table(k)) = &prm.ty else {
                return Err(unsupported(prm.name.span, "a law takes its row, dt and `[Kind]` tables; a check its row and tables"));
            };
            if !reads.iter().any(|(r, _)| *r == k.name) { reads.push((k.name.clone(), k.span)); }
        }
        Ok(())
    };
    let mut checks = vec![];
    let mut fields = vec![];
    let mut tier = Determinism::Det;
    for s in crate::check::motion_schedule(p)? {
        let f = s.f;
        let Some(target) = s.target else {
            return Err(unsupported(f.name.span, "motion target requires a kind"));
        };
        if target.name != kind.name.name {
            continue;
        }
        if let Some(law) = s.law {
            if !checks.is_empty() {
                return Err(Diag::new(
                    Code::E505,
                    s.span,
                    "law scheduled after its checks",
                    "schedule the row laws first and the checks last",
                ));
            }
            // Merge (lean2 one schedule reader + probe-rows table reads): lean2's loop over motion_schedule, with
            // probe-rows' rule that `[Kind]` parameters may follow `dt` (a law) or the row (a check).
            if f.params.len() < 2 || f.params[1].name.name != "dt" {
                return Err(unsupported(f.name.span, "row-local laws take the target row and dt"));
            }
            tables(f, 2)?;
            // A dispatch has the weakest determinism guarantee of any of its laws.
            tier = tier.max(law.tier);
            fields.extend(law.writes.iter().map(|w| w.name.clone()));
            laws.push(f);
        } else {
            if f.params.is_empty() {
                return Err(unsupported(f.name.span, "row-local checks take one target row"));
            }
            tables(f, 1)?;
            checks.push(f);
        }
    }
    if laws.is_empty() && checks.is_empty() {
        return Ok(None);
    }

    let (module, input) = output()?;
    let mut em = Emitter::new(p, types);
    em.fixed = fixed;
    em.header(abi::NONCE_PLACEHOLDER);
    em.row_header(kind);
    em.out = em.out.replace(
        "@group(1) @binding(0) var<storage, read> rows:",
        "@group(2) @binding(0) var<storage, read_write> rows:",
    );
    // A read of another kind binds its live rows; a read of this kind binds `loft_prev`, the copy `snapshot` takes
    // before each tick, so every row reads the same version (DESIGN §5.3). Without reads the text is unchanged.
    let previous = reads.iter().any(|(r, _)| *r == kind.name.name);
    for (j, (read, _)) in reads.iter().enumerate() {
        if *read == kind.name.name {
            em.out.push_str("@group(2) @binding(3) var<storage, read_write> loft_prev: array<f32>;\n");
            em.row_loader(kind, "Row", "loft_prev", &format!("loft_read_{read}"));
            em.tables.insert(read.clone(), format!("loft_read_{read}"));
        } else {
            em.foreign_table(p.kinds[read.as_str()], 1, 4 + j as u32);
        }
    }
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
    if has_events {
        em.out
            .push_str(&event_wgsl(2, 2, "loft_tick"));
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
    if previous {
        em.out.push_str(r#"
@compute @workgroup_size(64) fn snapshot(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x + loft_advance_args.first_row;
    if i >= loft_advance_args.end_row { return; }
    for (var w = 0u; w < STRIDE; w += 1u) { loft_prev[i * STRIDE + w] = rows[i * STRIDE + w]; }
}
"#);
    }
    let mut bindings = vec![
        (1, 0, DispatchResource::Arguments),
        (2, 0, DispatchResource::Rows),
    ];
    if !checks.is_empty() {
        bindings.push((2, 1, DispatchResource::Results));
    }
    if has_events {
        bindings.push((2, 2, DispatchResource::Outbox));
    }
    let mut dispatch_reads = vec![];
    for (j, (read, span)) in reads.iter().enumerate() {
        let own = *read == kind.name.name;
        bindings.push(if own { (2, 3, DispatchResource::Previous) } else { (1, 4 + j as u32, DispatchResource::Read(j as u32)) });
        dispatch_reads.push(DispatchRead { kind: read.clone(), input: if own { input.clone() } else { rows_of(read, *span)? }, previous: own });
    }
    Ok(Some((
        em.out,
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
            reads: dispatch_reads,
            lockstep: false,
        },
    )))
}

fn motion_params(program: &Program) -> Vec<ParamMeta> {
    const ENV_WORDS: usize = 12;
    let numeric = |i: usize, v| ParamMeta {
        id: ParamId(i as u32), name: i.to_string(), ty: IrTy::F32,
        offset_bytes: i as u32 * 4, default: [v, 0., 0., 0.], range: None,
    };
    if program.param_ranges.is_empty() {
        return program.params.iter().copied().enumerate().map(|(i, v)| numeric(i, v)).collect();
    }
    // Legacy environment words stay populated and receive private IDs that
    // cannot alias the reflected authored parameter IDs.
    let mut result: Vec<_> = program.params.iter().copied().take(ENV_WORDS).enumerate().map(|(i, v)| {
        ParamMeta { id: ParamId(u32::MAX - i as u32), ..numeric(i, v) }
    }).collect();
    result.extend(program.param_ranges.iter().enumerate().map(|(i, (name, range))| {
        let word = ENV_WORDS + i * 4;
        ParamMeta {
            id: ParamId(i as u32), name: name.clone(), ty: IrTy::F32,
            offset_bytes: word as u32 * 4, default: [program.params[word], 0., 0., 0.],
            range: range.map(|[lo, hi]| ([lo, 0., 0., 0.], [hi, 0., 0., 0.])),
        }
    }));
    result
}

/// A tick pass running `entry` of `module` over the rows of `instances`.
pub(crate) fn ticks_run(tier: Determinism, module: ModuleId, entry: &str, instances: TableId) -> PassRun {
    let (entry, max_ticks) = (entry.to_owned(), AdvanceArgs::MAX_TICKS);
    match tier {
        Determinism::Det => PassRun::ComputeFusedTicks { module, entry, max_ticks, instances },
        Determinism::NonDet => PassRun::ComputeNonDetTicks { module, entry, max_ticks, instances },
    }
}

fn table(id: TableId, stride_bytes: u32, capacity: u32, initial: Box<[u8]>) -> TableDecl {
    TableDecl { id, lifetime: TableLifetime::Static, format: ElemFormat::U32, stride_bytes, capacity, initial }
}

fn row_table<'a>(program: &'a Program, input: &'a DispatchRows, id: TableId) -> (&'a str, TableDecl) {
    let (name, rows, stride, words) = input.view(&program.kinds).expect("validated dispatch rows");
    (name, table(id, stride * 4, rows, words.iter().flat_map(|f| f.to_le_bytes()).collect()))
}

fn lut_table(program: &Program) -> TableDecl {
    table(TableId(4), 4, program.luts.len() as u32, program.luts.iter().flat_map(|f| f.to_le_bytes()).collect())
}

fn motion_table(program: &Program, step: &Dispatch, resource: DispatchResource) -> TableDecl {
    use DispatchResource as R;
    let (_, rows, stride, _) = step.input.view(&program.kinds).expect("validated dispatch");
    match resource {
        R::Rows => row_table(program, &step.input, ROWS).1,
        R::Arguments => table(TableId(1), 24, 1, Box::new([0; 24])),
        R::Results => table(RESULTS, step.checks.len() as u32 * 32, rows, Box::new([])),
        R::Outbox => table(TableId(3), 4, 4096, Box::new([])),
        R::Luts => lut_table(program),
        R::Read(j) => row_table(program, &step.reads[j as usize].input, TableId(5 + j)).1,
        R::Previous => table(TableId(5 + step.reads.len() as u32), stride * 4, rows, Box::new([])),
    }
}

fn motion_pass(
    bundle: &mut Bundle, program: &Program, step: &Dispatch, rows: TableId,
    mut bind: impl FnMut(&mut Bundle, DispatchResource) -> TableId,
) -> PassDecl {
    use DispatchResource as R;
    let module = ModuleId(bundle.modules.len() as u32);
    let wgsl = program.modules[step.module].wgsl.as_bytes();
    bundle.modules.push(ModuleBlob {
        id: module, hash: *blake3::hash(wgsl).as_bytes(), bytes: wgsl.into(), target: ShaderTarget::Wgsl,
    });
    let bindings = step.bindings.iter().map(|binding| {
        let access = match binding.resource {
            R::Arguments | R::Luts | R::Read(_) => BindAccess::ReadStorage,
            R::Rows | R::Results | R::Outbox | R::Previous => BindAccess::ReadWriteStorage,
        };
        PassBinding { group: binding.group as u8, slot: binding.slot, resource: bind(bundle, binding.resource), access }
    }).collect();
    PassDecl { id: PassId(0), wave: 0, queue: QueueClass::AsyncCompute, bindings,
        run: ticks_run(step.tier, module, &step.entry, rows) }
}

pub fn motion_bundle(program: &Program, step: &Dispatch, checkpoint_slots: u32) -> Bundle {
    let state = StateStoreDecl { table: ROWS, layout_hash: step.layout_hash,
        checkpoint_slots: if step.tier.is_det() { checkpoint_slots } else { 0 } };
    let mut bundle = Bundle { params: motion_params(program), state: Some(state), ..Default::default() };
    let pass = motion_pass(&mut bundle, program, step, ROWS, |bundle, resource| {
        let table = motion_table(program, step, resource);
        let id = table.id;
        bundle.tables.push(table);
        id
    });
    bundle.passes.push(pass);
    bundle
}

/// The state and optional verdict table of one step in a lockstep bundle.
pub struct GroupTables {
    pub rows: TableId,
    pub results: Option<TableId>,
}

/// Bundle the steps in program order, with snapshots before each step that reads its own previous rows.
pub fn motion_group(program: &Program, steps: &[usize]) -> (Bundle, Vec<GroupTables>) {
    use DispatchResource as R;
    let mut bundle = Bundle { params: motion_params(program), ..Default::default() };
    let push = |bundle: &mut Bundle, mut table: TableDecl| {
        table.id = TableId(bundle.tables.len() as u32);
        let id = table.id;
        bundle.tables.push(table);
        id
    };
    let rows: Vec<_> = steps.iter().map(|&i| {
        let (name, table) = row_table(program, &program.dispatches[i].input, ROWS);
        (name, push(&mut bundle, table))
    }).collect();
    let luts = push(&mut bundle, lut_table(program));
    let mut body = Vec::new();
    let mut out = Vec::new();
    for (&index, &(_, row)) in steps.iter().zip(&rows) {
        let step = &program.dispatches[index];
        let mut results = None;
        let mut pass = motion_pass(&mut bundle, program, step, row, |bundle, resource| {
            match resource {
                R::Rows => return row,
                R::Luts => return luts,
                R::Read(j) => {
                    let read = &step.reads[j as usize];
                    if let Some(&(_, live)) = rows.iter().find(|(name, _)| *name == read.kind.as_str()) {
                        return live;
                    }
                }
                R::Arguments | R::Results | R::Outbox | R::Previous => {}
            }
            let id = push(bundle, motion_table(program, step, resource));
            if resource == R::Results { results = Some(id); }
            id
        });
        if step.reads.iter().any(|r| r.previous) {
            let module = ModuleId(bundle.modules.len() as u32 - 1);
            body.push(PassDecl { id: PassId(body.len() as u32 + 1),
                run: ticks_run(step.tier, module, "snapshot", row), ..pass.clone() });
        }
        pass.id = PassId(body.len() as u32 + 1);
        body.push(pass);
        out.push(GroupTables { rows: row, results });
    }
    bundle.passes.push(PassDecl { id: PassId(0), wave: 0, queue: QueueClass::AsyncCompute, bindings: Box::new([]),
        run: PassRun::PassLoop { max_iters: AdvanceArgs::MAX_TICKS, cond_buf: None, body: body.into() } });
    (bundle, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_tier_keeps_nondet_in_either_order() {
        for laws in ["loose, exact", "exact, loose"] {
            let source = format!(r#"
                space World
                kind Agent @World () {{}}
                law loose(a: Agent, dt) nondet writes a.{{}} {{}}
                law exact(a: Agent, dt) writes a.{{}} {{}}
                schedule Main {{ run_laws({laws}) }}
            "#);
            let modules = [loft_parse::parse(0, &source, &mut 0).unwrap()];
            let p = Prog::index(&modules).unwrap();
            // The checker normally rejects mixed tiers. Exercise lowering directly.
            let (wgsl, dispatch) = motion_module(
                &p, &[], p.kinds["Agent"], &Default::default(),
                || Ok((0, DispatchRows::Drawable(0))),
                |_, _| unreachable!("no reads"),
            ).unwrap().unwrap();
            assert_eq!(dispatch.tier, Determinism::NonDet);
            assert!(!wgsl.contains("fn det_sin"));
        }
    }
}
