//! The layout (plan §1.10): row resources packed into as few storage buffers as hold them, first fit in resource
//! order, each at most one binding (TS `pack`, src/compile/tables.ts). Resources a kernel writes and resources only
//! read are packed apart, since a binding has one access mode. A stepped resource holds every state it keeps.
//! Textures and the frame uniform are bound on their own.
use crate::kernel::{Plan, kept};
use loft_ir::Error;
use loft_ir::bundle::{Shape, When};
use loft_ir::error::SceneFault;

/// Storage buffers a shader stage binds beside the prelude's three (WebGPU's default `maxStorageBuffersPerShaderStage`, 8).
pub const STORAGE_BUFFERS: usize = 5;
/// `maxStorageBufferBindingSize` default, 128 MiB, in vec4s.
pub const BINDING_VEC4S: u64 = (128 << 20) / 16;

pub struct Layout {
    pub buffers: Vec<Buffer>,
    /// Per resource of the plan.
    pub at: Vec<Place>,
}

pub struct Buffer {
    pub written: bool,
    pub vec4s: u64,
}

pub enum Place {
    Rows { buffer: u32, offset: u64 },
    Bound,
}

pub fn layout(plan: &Plan) -> Result<Layout, Error> {
    let refuse = |what: String| Error::Scene { site: None, fault: SceneFault::Unsupported { what: what.into() } };
    let mut states = vec![1u64; plan.resources.len()];
    let mut written = vec![false; plan.resources.len()];
    for k in &plan.kernels {
        written[k.writes as usize] = true;
        if let When::Steps { .. } = k.when {
            states[k.writes as usize] = u64::from(kept(&k.when));
        }
    }
    let mut buffers: Vec<Buffer> = Vec::new();
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
        let b = match buffers.iter().position(|b| b.written == written[r] && b.vec4s + n <= BINDING_VEC4S) {
            Some(b) => b,
            None => {
                buffers.push(Buffer { written: written[r], vec4s: 0 });
                buffers.len() - 1
            }
        };
        at.push(Place::Rows { buffer: b as u32, offset: buffers[b].vec4s });
        buffers[b].vec4s += n;
    }
    match buffers.len() {
        0..=STORAGE_BUFFERS => Ok(Layout { buffers, at }),
        n => Err(refuse(format!(
            "the resources need {n} storage buffers; a shader stage binds {STORAGE_BUFFERS} beside the prelude: keep fewer states or rows"
        ))),
    }
}
