//! Lane W. The author-side recording and the inputs to pure lowering.
mod decode;
mod types;

use crate::error::Error;
use loft_ir::graph::Graph;
use loft_ir::wire::{self, Job};
pub use types::*;

pub struct Input {
    pub job: Job,
    port: decode::Port,
}

impl Input {
    /// The graph: nodes, declarations and the program roots, as loft-ir decoded them.
    pub fn graph(&self) -> &Graph {
        &self.job.graph
    }
    pub fn things(&self) -> &[Thing] {
        &self.port.things
    }
    pub fn effects(&self) -> &[Effect] {
        &self.port.effects
    }
    pub fn style_emits(&self) -> &[EmitRequest] {
        &self.port.style
    }
    pub fn lighting(&self) -> Option<&Lighting> {
        self.port.lighting.as_ref()
    }
    pub fn lamp_passes(&self) -> &[LampPass] {
        &self.port.lamps
    }
    pub fn posts(&self) -> &[Post] {
        self.port.posts.as_deref().unwrap_or_default()
    }
    pub fn compute_passes(&self) -> &[ComputePass] {
        &self.port.compute
    }
    pub fn recurrences(&self) -> &[Recurrence] {
        &self.port.recurrences
    }
    /// Pass ordinal order, as used by `graph::Read::Pass`. `discovery_order` gives the compiler's visit order.
    pub fn pass_defs(&self) -> &[PassDef] {
        &self.port.passes
    }
    pub fn scene(&self) -> Option<&SceneDecl> {
        self.port.scene.as_ref()
    }
    pub fn cameras(&self) -> &[Camera] {
        &self.port.cameras
    }
    pub fn authored(&self) -> &[Authored] {
        &self.port.authored
    }
    pub fn cast(&self) -> Option<&Cast> {
        self.port.cast.as_ref()
    }
    pub fn raw_flags(&self, raw: u32) -> Option<&RawFlags> {
        self.port.raws.get(raw as usize)?.as_ref()
    }
}

pub fn decode(bytes: &[u8]) -> Result<Input, Error> {
    let job = wire::decode(bytes)?;
    let port = decode::Port::decode(&job)?;
    Ok(Input { job, port })
}

/// The canonical print of the job, also written by src/record/print.ts as `probe.wire`.
pub fn print(input: &Input) -> Result<String, Error> {
    Ok(loft_ir::print::print_job(&input.job)?)
}
