//! Fixed-step passes and their buffer declarations.
use crate::program::{Dispatch, DispatchResource, Program};
pub use loft_abi::advance;
use loft_abi::AdvanceArgs;
pub use loft_emit::events::{event_tag, event_wgsl};
use loft_ir::*;

pub const ROWS: TableId = TableId(0);
pub const RESULTS: TableId = TableId(2);

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

pub(crate) fn tick_run(step: &Dispatch, module: ModuleId, instances: TableId) -> PassRun {
    use loft_parse::ast::Determinism;
    let entry = step.entry.clone();
    let max_ticks = AdvanceArgs::MAX_TICKS;
    match step.tier {
        Determinism::Det => PassRun::ComputeFusedTicks { module, entry, max_ticks, instances },
        Determinism::NonDet => PassRun::ComputeNonDetTicks { module, entry, max_ticks, instances },
    }
}

pub fn motion_bundle(program: &Program, step: &Dispatch, checkpoint_slots: u32) -> Bundle {
    let (_, rows, stride, words) = step.input.view(&program.kinds).expect("validated dispatch");
    let wgsl = &program.modules[step.module].wgsl;
    let mut b = Bundle {
        modules: vec![ModuleBlob {
            id: ModuleId(0),
            hash: *blake3::hash(wgsl.as_bytes()).as_bytes(),
            bytes: wgsl.as_bytes().into(),
            target: ShaderTarget::Wgsl,
        }],
        state: Some(StateStoreDecl {
            table: ROWS,
            layout_hash: step.layout_hash,
            checkpoint_slots: if step.tier.is_det() { checkpoint_slots } else { 0 },
        }),
        params: motion_params(program),
        ..Default::default()
    };
    let mut bindings = vec![];
    for binding in &step.bindings {
        let (id, access, count, stride, bytes) = match binding.resource {
            DispatchResource::Rows => (
                ROWS,
                BindAccess::ReadWriteStorage,
                rows,
                stride * 4,
                words
                    .iter()
                    .flat_map(|f| f.to_le_bytes())
                    .collect::<Vec<_>>(),
            ),
            DispatchResource::Arguments => {
                (TableId(1), BindAccess::ReadStorage, 1, 24, vec![0; 24])
            }
            DispatchResource::Results => (
                RESULTS,
                BindAccess::ReadWriteStorage,
                rows,
                step.checks.len() as u32 * 32,
                vec![],
            ),
            DispatchResource::Luts => (
                TableId(4), BindAccess::ReadStorage, program.luts.len() as u32, 4,
                program.luts.iter().flat_map(|f| f.to_le_bytes()).collect(),
            ),
            DispatchResource::Outbox => (TableId(3), BindAccess::ReadWriteStorage, 4096, 4, vec![]),
            // A row-local dispatch reads only kinds no law writes (others run in lockstep, `motion_group`).
            DispatchResource::Read(j) => {
                let (_, n, stride, words) = step.reads[j as usize].input.view(&program.kinds).expect("validated read");
                (TableId(5 + j), BindAccess::ReadStorage, n, stride * 4, words.iter().flat_map(|f| f.to_le_bytes()).collect())
            }
            DispatchResource::Previous => (TableId(5 + step.reads.len() as u32), BindAccess::ReadWriteStorage, rows, stride * 4, vec![]),
        };
        b.tables.push(TableDecl {
            id,
            lifetime: TableLifetime::Static,
            format: ElemFormat::U32,
            stride_bytes: stride,
            capacity: count,
            initial: bytes.into(),
        });
        bindings.push(PassBinding {
            group: binding.group as u8,
            slot: binding.slot,
            resource: id,
            access,
        });
    }
    b.passes.push(PassDecl {
        id: PassId(0),
        wave: 0,
        queue: QueueClass::AsyncCompute,
        bindings: bindings.into(),
        run: tick_run(step, ModuleId(0), ROWS),
    });
    b
}

/// A tick pass running `entry` of `module` over the rows of `instances`.
pub(crate) fn ticks_run(tier: loft_parse::ast::Determinism, module: ModuleId, entry: &str, instances: TableId) -> PassRun {
    use loft_parse::ast::Determinism;
    let (entry, max_ticks) = (entry.to_owned(), AdvanceArgs::MAX_TICKS);
    match tier {
        Determinism::Det => PassRun::ComputeFusedTicks { module, entry, max_ticks, instances },
        Determinism::NonDet => PassRun::ComputeNonDetTicks { module, entry, max_ticks, instances },
    }
}

/// Where each lockstep step's state lands in a `motion_group` bundle: its rows, and its verdicts if it has checks.
pub struct GroupTables {
    pub rows: TableId,
    pub results: Option<TableId>,
}

/// The lockstep dispatches `steps` (indices into `program.dispatches`, in program order) as one standalone bundle: a
/// `PassLoop` of tick passes, which the runner advances one tick per iteration, each pass in order (DESIGN §5.3). A
/// read binds the reading step's view of the read kind: the live rows of the step that writes it, or a copy of its
/// constant rows, or (`previous`) the snapshot the step's own `snapshot` entry takes before each tick.
pub fn motion_group(program: &Program, steps: &[usize]) -> (Bundle, Vec<GroupTables>) {
    let mut b = Bundle { params: motion_params(program), ..Default::default() };
    let table = |b: &mut Bundle, stride_bytes: u32, capacity: u32, initial: Vec<u8>| {
        let id = TableId(b.tables.len() as u32);
        b.tables.push(TableDecl { id, lifetime: TableLifetime::Static, format: ElemFormat::U32, stride_bytes, capacity, initial: initial.into() });
        id
    };
    let bytes = |words: &[f32]| words.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>();
    let group: Vec<&Dispatch> = steps.iter().map(|&i| &program.dispatches[i]).collect();
    let views: Vec<_> = group.iter().map(|step| step.input.view(&program.kinds).expect("validated dispatch")).collect();
    let rows: Vec<TableId> = views.iter().map(|&(_, n, stride, words)| table(&mut b, stride * 4, n, bytes(words))).collect();
    let luts = table(&mut b, 4, program.luts.len() as u32, bytes(&program.luts));
    let mut out = vec![];
    let mut body = vec![];
    for (k, step) in group.iter().enumerate() {
        let wgsl = &program.modules[step.module].wgsl;
        let module = ModuleId(k as u32);
        b.modules.push(ModuleBlob { id: module, hash: *blake3::hash(wgsl.as_bytes()).as_bytes(), bytes: wgsl.as_bytes().into(), target: ShaderTarget::Wgsl });
        let (_, n, stride, _) = views[k];
        let mut results = None;
        let mut bindings = vec![];
        for binding in &step.bindings {
            let (resource, access) = match binding.resource {
                DispatchResource::Rows => (rows[k], BindAccess::ReadWriteStorage),
                DispatchResource::Arguments => (table(&mut b, 24, 1, vec![0; 24]), BindAccess::ReadStorage),
                DispatchResource::Results => {
                    let id = table(&mut b, step.checks.len() as u32 * 32, n, vec![]);
                    results = Some(id);
                    (id, BindAccess::ReadWriteStorage)
                }
                DispatchResource::Luts => (luts, BindAccess::ReadStorage),
                DispatchResource::Outbox => (table(&mut b, 4, 4096, vec![]), BindAccess::ReadWriteStorage),
                DispatchResource::Read(j) => {
                    let read = &step.reads[j as usize];
                    let live = views.iter().position(|v| v.0 == read.kind).map(|i| rows[i]);
                    let id = live.unwrap_or_else(|| {
                        let (_, n, stride, words) = read.input.view(&program.kinds).expect("validated read");
                        table(&mut b, stride * 4, n, bytes(words))
                    });
                    (id, BindAccess::ReadStorage)
                }
                DispatchResource::Previous => (table(&mut b, stride * 4, n, vec![]), BindAccess::ReadWriteStorage),
            };
            bindings.push(PassBinding { group: binding.group as u8, slot: binding.slot, resource, access });
        }
        let bindings: Box<[PassBinding]> = bindings.into();
        if step.reads.iter().any(|r| r.previous) {
            body.push(PassDecl { id: PassId(body.len() as u32 + 1), wave: 0, queue: QueueClass::AsyncCompute,
                bindings: bindings.clone(), run: ticks_run(step.tier, module, "snapshot", rows[k]) });
        }
        body.push(PassDecl { id: PassId(body.len() as u32 + 1), wave: 0, queue: QueueClass::AsyncCompute,
            bindings, run: ticks_run(step.tier, module, &step.entry, rows[k]) });
        out.push(GroupTables { rows: rows[k], results });
    }
    b.passes.push(PassDecl { id: PassId(0), wave: 0, queue: QueueClass::AsyncCompute, bindings: Box::new([]),
        run: PassRun::PassLoop { max_iters: AdvanceArgs::MAX_TICKS, cond_buf: None, body: body.into() } });
    (b, out)
}
