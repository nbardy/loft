//! Lane W. The job src/record/port.ts writes, decoded into the typed input the compiler reads. Today that job is the
//! wire-4 job (decoded by loft-ir); the lane adds the emission tape and the scene decls of the plan's gap list, and
//! keeps `graph()` (lanes G1 and G2 read the program roots through it).
use crate::error::Error;

pub struct Input {
    pub job: loft_ir::wire::Job,
}

impl Input {
    /// The job's graph: nodes, decls and the program roots, as loft-ir decoded them.
    pub fn graph(&self) -> &loft_ir::graph::Graph {
        &self.job.graph
    }
}

pub fn decode(bytes: &[u8]) -> Result<Input, Error> {
    Ok(Input { job: loft_ir::wire::decode(bytes)? })
}

/// The canonical print of the job (loft-ir print.rs = src/record/print.ts `printJob`): the `probe.wire` section.
pub fn print(input: &Input) -> Result<String, Error> {
    Ok(loft_ir::print::print(input.graph())?)
}
