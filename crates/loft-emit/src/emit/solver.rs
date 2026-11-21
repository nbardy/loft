//! Lower an authored state solver to the ordinary fixed-tick dispatch ABI.
use super::*;
use loft_check::check::motion_source::{Distance, SolverConfig, StateMemory, Step, Strategy, scheduled};
use loft_check::program::{DispatchBinding, DispatchResource, DispatchRows};
use loft_abi::AdvanceArgs;

pub fn lower(
    p: &Prog<'_>,
    types: &mut Vec<Ty>,
    module: usize,
    solver: &SolverDecl,
    initial_words: impl Fn(&Prog<'_>, &mut Vec<Ty>, &Expr) -> Res<Vec<f32>>,
) -> Res<(String, loft_check::program::Dispatch)> {
    let (table, config, steps) = scheduled(p, types, solver)?;
    let SolverConfig {
        strategy,
        memory,
        substeps,
        iterations,
    } = config;
    let mut words = Vec::new();
    let mut layout = blake3::Hasher::new();
    let mut declarations = String::new();
    let mut state = String::new();
    let mut load = String::new();
    let mut store = String::new();
    let mut fields = Vec::new();
    let mut aligned_bytes = 0u32;

    for field in &table.fields {
        let ty = loft_check::check::Ck::new(p, types).resolve(&field.ty, true)?;
        let (element, count) = match &ty {
            Ty::Array(element, count) => (element.as_ref(), *count),
            other => (other, 1),
        };
        let width = match element {
            Ty::F32 | Ty::Geo(..) | Ty::VecN(2..=4) => {
                element.words().expect("packed state element")
            }
            _ => {
                return Err(refuse(
                    field.name.span,
                    "solver state requires float or vector elements",
                ));
            }
        };
        let name = &field.name.name;
        let symbol = wgsl_name(name);
        let ty_wgsl = wty(&ty);
        let offset = words.len() as u32;
        let len = count
            .checked_mul(width)
            .filter(|n| *n <= 65_536)
            .ok_or_else(|| refuse(field.name.span, "solver state column exceeds 65,536 words"))?;
        layout.update(format!("{name}:{ty}:{offset}:{len}\n").as_bytes());
        let align = width.next_power_of_two() * 4;
        aligned_bytes = aligned_bytes.next_multiple_of(align) + count * align;
        let initial = match &field.default {
            None => vec![0.; width as usize],
            Some(e) => {
                let initial = initial_words(p, types, e)?;
                if initial.len() != width as usize || !initial.iter().all(|x| x.is_finite()) {
                    return Err(refuse(
                        e.span,
                        "state initializer needs one finite element of the declared width",
                    ));
                }
                initial
            }
        };
        words.extend(initial.into_iter().cycle().take(len as usize));
        // With one invocation per workgroup, shared state can be promoted
        // to that invocation. Private variables also run in the CPU twin.
        writeln!(declarations, "var<private> {symbol}: {ty_wgsl};").unwrap();
        writeln!(state, "    r_{name}: {ty_wgsl},").unwrap();
        let writable = solver.writes.as_ref().is_some_and(|(_, fields)| {
            fields.is_empty() || fields.iter().any(|field| field.name == *name)
        });
        let (value, index) = if matches!(ty, Ty::Array(..)) {
            writeln!(load, "for (var k = 0u; k < {count}u; k++) {{").unwrap();
            if writable {
                writeln!(store, "for (var k = 0u; k < {count}u; k++) {{").unwrap();
            }
            (
                format!("{symbol}[k]"),
                format!("base + {offset}u + k * {width}u"),
            )
        } else {
            if ty == Ty::F32 && writable {
                fields.push((name.clone(), offset));
            }
            (symbol, format!("base + {offset}u"))
        };
        let lanes = (0..width)
            .map(|i| format!("solver_rows[{index} + {i}u]"))
            .collect::<Vec<_>>();
        writeln!(load, "{value} = {}({});", wty(element), lanes.join(", ")).unwrap();
        if writable {
            for (i, lane) in lanes.iter().enumerate() {
                let word = if width == 1 {
                    value.clone()
                } else {
                    format!("{value}[{i}u]")
                };
                writeln!(store, "{lane} = {word};").unwrap();
            }
            if matches!(ty, Ty::Array(..)) {
                store.push_str("}\n");
            }
        }
        if matches!(ty, Ty::Array(..)) {
            load.push_str("}\n");
        }
    }

    if matches!(memory, StateMemory::Workgroup) && aligned_bytes > 16 * 1024 {
        return Err(refuse(
            table.name.span,
            "solver state exceeds 16 KiB of portable workgroup memory",
        ));
    }
    let mut constraints = Vec::new();
    let mut body = Vec::new();
    for step in steps {
        match step {
            Step::Statement(stmt) => {
                if !constraints.is_empty() {
                    return Err(refuse(
                        solver.name.span,
                        "solver statements must precede its iterative constraints",
                    ));
                }
                body.push(stmt.clone());
            }
            Step::Distance(edge) => constraints.push(edge),
        }
    }
    let mut dependencies = body.clone();
    dependencies.extend(
        constraints
            .iter()
            .flat_map(|edge| edge.args)
            .cloned()
            .map(Stmt::Expr),
    );
    let root = FnDecl {
        comptime: false,
        attrs: vec![],
        name: solver.name.clone(),
        raw: None,
        params: vec![],
        ret: None,
        body: Block {
            stmts: dependencies,
        },
    };
    let (functions, constants) = reachable(p, &[&root], &[]);
    let mut calls = References::function(&root).calls;
    let mut has_events = false;
    for f in &functions {
        if f.refuses_unverified_raw() {
            return Err(nondeterministic(f.name.span));
        }
        calls.extend(References::function(f).calls);
    }
    for call in calls {
        has_events |= call.name == "emit_event";
        if !p.fns.contains_key(call.name.as_str())
            && matches!(motion_math(&call.name), MotionMath::Unavailable)
        {
            return Err(nondeterministic(call.span));
        }
    }

    let mut em = Emitter::new(p, types);
    em.motion = true;
    em.header(abi::NONCE_PLACEHOLDER);
    em.out.push_str(loft_abi::DET_WGSL);
    em.out.push_str(AdvanceArgs::WGSL);
    em.out.push_str(
        "struct SolverArgs { ticks: AdvanceArgs, first_row: u32, end_row: u32, instance_id: u32 }\n\
         @group(1) @binding(0) var<storage, read> loft_advance_args: SolverArgs;\n\
         @group(2) @binding(0) var<storage, read_write> solver_rows: array<f32>;\n\
         var<private> loft_tick: u32;\n",
    );
    if has_events {
        em.out
            .push_str(&crate::events::event_wgsl(2, 2, "loft_tick"));
    }
    writeln!(em.out, "struct Row {{\n{state}}}\n{declarations}").unwrap();
    em.constants(&constants);
    for f in functions {
        em.func(f, &format!("motion_{}", fn_symbol(&f.name.name)));
    }
    em.locals = table.fields.iter().map(|f| f.name.name.clone()).collect();
    em.locals.push("dt".into());
    writeln!(em.out, "fn solver_substep(loft_local_dt: f32) {{").unwrap();
    // Keep top-level locals alive through the edge sweeps.
    for stmt in &body {
        em.stmt(stmt, 1);
    }
    for (i, edge) in constraints.iter().enumerate() {
        writeln!(
            em.out,
            "var solver_lambda_{i}: array<f32, {}>;",
            edge.end - edge.start
        )
        .unwrap();
    }
    if !constraints.is_empty() {
        writeln!(
            em.out,
            "for (var iteration = 0u; iteration < {iterations}u; iteration++) {{"
        )
        .unwrap();
        for (i, edge) in constraints.iter().enumerate() {
            match strategy {
                Strategy::Symmetric => {
                    sweep(&mut em, edge, i, Sweep::Forward);
                    sweep(&mut em, edge, i, Sweep::Reverse);
                }
                Strategy::Colored(n) => {
                    writeln!(em.out, "for (var color = 0u; color < {n}u; color++) {{").unwrap();
                    sweep(&mut em, edge, i, Sweep::Colors(n));
                    em.out.push_str("}\n");
                }
            }
        }
        em.out.push_str("}\n");
    }
    let stride = words.len() as u32;
    writeln!(
        em.out,
        "
}}
@compute @workgroup_size(1) fn advance(@builtin(global_invocation_id) gid: vec3u) {{
    let i = gid.x + loft_advance_args.first_row;
    if i >= loft_advance_args.end_row {{ return; }}
    let base = i * {stride}u;
    {load}
    let dt = 1.0 / 120.0;
    let first = loft_advance_args.ticks.from_tick;
    let count = loft_advance_args.ticks.to_tick - first;
    for (var step = 0u; step < {}u; step++) {{
        if step >= count {{ break; }}
        loft_tick = first + step + 1u;
        loft_tick_time = f32(loft_tick) * dt;
        for (var sub = 0u; sub < {substeps}u; sub++) {{
            solver_substep(dt / {substeps}.0);
        }}
    }}
    {store}
}}",
        AdvanceArgs::MAX_TICKS
    )
    .unwrap();
    let mut bindings = vec![
        DispatchBinding {
            group: 1,
            slot: 0,
            resource: DispatchResource::Arguments,
        },
        DispatchBinding {
            group: 2,
            slot: 0,
            resource: DispatchResource::Rows,
        },
    ];
    if has_events {
        bindings.push(DispatchBinding {
            group: 2,
            slot: 2,
            resource: DispatchResource::Outbox,
        });
    }
    Ok((
        em.out,
        loft_check::program::Dispatch {
            tier: Determinism::Det,
            module,
            entry: AdvanceArgs::ENTRY.into(),
            workgroup_x: 1,
            input: DispatchRows::Table {
                name: table.name.name.clone(),
                n_rows: 1,
                stride,
                words,
            },
            fields,
            layout_hash: *layout.finalize().as_bytes(),
            bindings,
            checks: vec![],
        },
    ))
}

enum Sweep {
    Forward,
    Reverse,
    Colors(u32),
}

fn sweep(em: &mut Emitter<'_, '_>, edge: &Distance<'_>, id: usize, sweep: Sweep) {
    let [a, b] = edge.endpoints.map(|binder| wgsl_name(&binder.name));
    let Distance {
        start, end, field, ..
    } = *edge;
    let field = wgsl_name(field);
    // Colors partition the path edges. One invocation owns the row and
    // completes each color before the next color sees any updated endpoint.
    let loop_head = match sweep {
        Sweep::Forward => format!("for (var {a} = {start}u; {a} < {end}u; {a}++) {{"),
        Sweep::Reverse => {
            format!("for (var next = {end}u; next > {start}u; next--) {{ let {a} = next - 1u;")
        }
        Sweep::Colors(n) => {
            format!("for (var {a} = {start}u + color; {a} < {end}u; {a} += {n}u) {{")
        }
    };
    writeln!(em.out, "{loop_head}\nlet {b} = {a} + 1u;").unwrap();
    let mark = em.locals.len();
    em.locals.extend(edge.endpoints.map(|n| n.name.clone()));
    let [im_a, im_b, rest, alpha] = edge.args.map(|arg| em.expr(arg));
    em.locals.truncate(mark);
    writeln!(
        em.out,
        "
    let im_a = {im_a};
    let im_b = {im_b};
    let rest = {rest};
    let alpha = {alpha};
    let p_a = {field}[{a}];
    let p_b = {field}[{b}];
    let d = p_b - p_a;
    let distance = sqrt(fma(d.x, d.x, fma(d.y, d.y, d.z * d.z)));
    let denom = im_a + im_b + alpha;
    if denom > 0.0 {{
        let slot = {a} - {start}u;
        let lambda = solver_lambda_{id}[slot];
        let delta = fma(-alpha, lambda, rest - distance) / denom;
        solver_lambda_{id}[slot] = lambda + delta;
        // A coincident pair has no gradient. Choose +X deterministically.
        let separated = distance > 0.0;
        let axis = select(vec3f(1.0, 0.0, 0.0), d / select(1.0, distance, separated), separated);
        {field}[{a}] = fma(axis, vec3f(-im_a * delta), p_a);
        {field}[{b}] = fma(axis, vec3f(im_b * delta), p_b);
    }}
}}"
    )
    .unwrap();
}

fn refuse(span: Span, msg: &str) -> Diag {
    Diag::new(Code::E405, span, msg, "use a packed GPU solver state")
}

fn nondeterministic(span: Span) -> Diag {
    Diag::new(
        Code::E504,
        span,
        "solver reaches an operation without a deterministic implementation",
        "use deterministic Loft math or verified WGSL",
    )
}
