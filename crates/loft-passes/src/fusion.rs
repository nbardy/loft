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
