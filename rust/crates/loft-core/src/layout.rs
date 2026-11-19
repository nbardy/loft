//! The layout (plan §1.10): row resources packed into as few storage buffers as hold them, first fit in resource
//! order, each at most one binding (TS `pack`, src/compile/tables.ts). Every shader stage binds the same packs at
//! `@group(1) @binding(p)` and reads a resource at its constant vec4 offset, so a scene with 600 tables binds as many
//! buffers as one with 6 (lean Rust draw note, 2026-11-19: one storage binding per table refused 205 of 232 corpus bundles
//! at WebGPU's default of 8 per stage).
//!
//! - Resources a kernel writes and resources only read are packed apart: a compute pipeline binds a written pack
//!   read-write, and WebGPU refuses one buffer bound both read-only and read-write in one dispatch.
//! - A stepped resource has a pack of its own, holding every state it keeps: the host binds each step's previous state
//!   in its place.
//! - Textures and the frame uniform are bound on their own.
use crate::kernel::{Plan, kept};
use loft_ir::Error;
use loft_ir::bundle::{Access, Pack, Shape, When};
use loft_ir::error::SceneFault;

/// Storage buffers a shader stage binds: WebGPU's default `maxStorageBuffersPerShaderStage` is 8, and a kernel binds
/// the buffer it writes once more at `@group(2)`.
pub const STORAGE_BUFFERS: usize = 5;
/// `maxStorageBufferBindingSize` default, 128 MiB, in vec4s.
pub const BINDING_VEC4S: u64 = (128 << 20) / 16;

pub struct Layout {
    pub packs: Vec<Pack>,
    /// Per resource of the plan.
    pub at: Vec<Place>,
}

/// Where a resource lives.
#[derive(Clone, Copy, Debug)]
pub enum Place {
    /// In pack `pack`, from vec4 `offset`.
    Rows { pack: u32, offset: u64 },
    /// Bound on its own: a texture or the frame.
    Bound,
}

/// What a pack may hold, decided per resource.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Read,
    Write,
    /// A stepped resource: alone in its pack.
    Stepped,
}

pub fn layout(plan: &Plan) -> Result<Layout, Error> {
    with_outputs(plan, &[])
}

pub fn with_outputs(plan: &Plan, outputs: &[u32]) -> Result<Layout, Error> {
    let refuse = |what: String| Error::Scene { site: None, fault: SceneFault::Unsupported { what: what.into() } };
    let mut kind = vec![Kind::Read; plan.resources.len()];
    for &output in outputs {
        kind[output as usize] = Kind::Stepped;
    }
    let mut states = vec![1u64; plan.resources.len()];
    for k in &plan.kernels {
        let w = k.writes as usize;
        match k.when {
            When::Steps { .. } => {
                kind[w] = Kind::Stepped;
                states[w] = u64::from(kept(&k.when));
            }
            When::Load | When::Frame if kind[w] == Kind::Read => kind[w] = Kind::Write,
            When::Load | When::Frame => {}
        }
    }
    let mut packs: Vec<(Kind, Pack)> = Vec::new();
    let mut at = Vec::with_capacity(plan.resources.len());
    for (r, res) in plan.resources.iter().enumerate() {
        let n = match res.shape {
            Shape::Rows { rows, lanes } => u64::from(rows) * u64::from(lanes) * states[r],
            Shape::Texture { .. } | Shape::ViewSized { .. } | Shape::Frame => {
                at.push(Place::Bound);
                continue;
            }
        };
        if n > BINDING_VEC4S {
            return Err(refuse(format!(
                "resource {r} needs {n} vec4s; one storage binding holds {BINDING_VEC4S}: keep fewer states or rows"
            )));
        }
        let fits = |(k, p): &(Kind, Pack)| *k == kind[r] && kind[r] != Kind::Stepped && p.vec4s + n <= BINDING_VEC4S;
        let p = match packs.iter().position(fits) {
            Some(p) => p,
            None => {
                let access = match kind[r] {
                    Kind::Read => Access::Read,
                    Kind::Write | Kind::Stepped => Access::Write,
                };
                packs.push((kind[r], Pack { access, vec4s: 0, holds: Box::new([]) }));
                packs.len() - 1
            }
        };
        let pack = &mut packs[p].1;
        at.push(Place::Rows { pack: p as u32, offset: pack.vec4s });
        pack.holds = pack.holds.iter().copied().chain([(r as u32, pack.vec4s)]).collect();
        pack.vec4s += n;
    }
    match packs.len() {
        0..=STORAGE_BUFFERS => Ok(Layout {
            // A pack of empty tables still binds: a zero-sized storage binding is refused.
            packs: packs.into_iter().map(|(_, p)| Pack { vec4s: p.vec4s.max(1), ..p }).collect(),
            at,
        }),
        n => Err(refuse(format!(
            "the resources need {n} storage buffers; a shader stage binds {STORAGE_BUFFERS}: keep fewer states or rows"
        ))),
    }
}
