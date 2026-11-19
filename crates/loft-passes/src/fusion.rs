//! Fixed-step passes and their buffer declarations.
use crate::program::{Dispatch, DispatchResource, Program};
pub use loft_abi::advance;
use loft_abi::{AdvanceArgs, VerdictReason};
use loft_ir::*;

pub const ROWS: TableId = TableId(0);
pub const RESULTS: TableId = TableId(2);

pub(crate) fn reason(name: &str) -> Option<VerdictReason> {
    Some(match name {
        "IterCap" => VerdictReason::IterCap,
        "NonFinite" => VerdictReason::NonFinite,
        "MissingSurface" => VerdictReason::MissingSurface,
        "MissingFrame" => VerdictReason::MissingFrame,
        "SolveFailed" => VerdictReason::SolveFailed,
        _ => return None,
    })
}

/// Stable symbol word also used by the host when looking up an authored tag.
pub fn event_tag(name: &str) -> u32 {
    u32::from_le_bytes(
        blake3::hash(name.as_bytes()).as_bytes()[..4]
            .try_into()
            .unwrap(),
    )
}

/// Bind the returned outbox as read/write storage in the pass declaration.
/// `tick` is the tick expression in the writer's scope.
pub fn event_wgsl(group: u8, slot: u32, tick: &str) -> String {
    format!(
        r#"
struct OutboxEvent {{ tag: u32, instance_id: u32, site_id: u32, tick: u32, payload: vec4<f32> }}
struct OutboxHeader {{ write_count: atomic<u32>, capacity: u32, dropped: atomic<u32>, frame_idx: u32 }}
struct Outbox {{ header: OutboxHeader, events: array<OutboxEvent> }}
@group({group}) @binding({slot}) var<storage, read_write> loft_outbox: Outbox;
fn emit_event(tag: u32, instance_id: u32, site_id: u32, payload: vec4<f32>) {{
    let slot = atomicAdd(&loft_outbox.header.write_count, 1u);
    if slot < loft_outbox.header.capacity {{
        loft_outbox.events[slot] = OutboxEvent(tag, instance_id, site_id, {tick}, payload);
    }} else {{
        atomicAdd(&loft_outbox.header.dropped, 1u);
    }}
}}
"#
    )
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
            checkpoint_slots,
        }),
        params: program
            .params
            .iter()
            .enumerate()
            .map(|(i, &v)| ParamMeta {
                id: ParamId(i as u32),
                name: i.to_string(),
                ty: IrTy::F32,
                offset_bytes: i as u32 * 4,
                default: [v, 0., 0., 0.],
                range: None,
            })
            .collect(),
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
        run: PassRun::ComputeFusedTicks {
            module: ModuleId(0),
            entry: step.entry.clone(),
            max_ticks: AdvanceArgs::MAX_TICKS,
            instances: ROWS,
        },
    });
    b
}
