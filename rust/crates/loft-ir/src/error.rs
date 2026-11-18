//! The one engine error (plan §1.8). Exit codes keep today's numbers (src/cli/verdict.ts): internal 1, usage 2,
//! load 3, scene 4, environment 6, budget 7. Exit 5 (`check`) is a verdict, not an error.
use crate::graph::{Program, Site, Ty};
use serde::Serialize;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Error {
    /// A bad request (a serve line, a CLI ask).
    Usage(Box<str>),
    /// The author's fault, at the author's line where the job recorded one.
    Scene { site: Option<Site>, fault: SceneFault },
    /// The job asked for more than its budget, counted in work units so the refusal is deterministic.
    Budget { meter: Meter, limit: u64, spent: u64 },
    /// A bundle file the host refuses.
    Load(LoadFault),
    /// The machine: a device lost, a library missing or stale.
    Environment(Box<str>),
    /// A Loft bug, never retried as the author's.
    Internal(Internal),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum SceneFault {
    /// An operand or result of the wrong type: `want` is what the rule needs, `got` what was given.
    Type {
        want: Ty,
        got: Ty,
    },
    Unsupported {
        what: Box<str>,
    },
    Infeasible {
        a: Site,
        b: Site,
    },
    Unsolved {
        fix: Box<str>,
    },
    Unreadable,
    /// A leaf (time, a param, an input, a binder) was given a value that is not finite: the node.
    NotFinite {
        node: u32,
    },
    Wgsl {
        line: u32,
        msg: Box<str>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Meter {
    Nodes,
    TapeOps,
    Boxes,
    GpuNanos,
}

/// One job's ceilings in work units, indexed by `Meter` (plan §7: training budgets as typed exits). A request sets
/// them, so the same job under the same limits refuses at the same count on every machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits(pub [u64; 4]);

impl Limits {
    /// `Budget` when `spent` is over `meter`'s limit.
    pub fn spend(&self, meter: Meter, spent: u64) -> Result<(), Error> {
        let limit = self.0[meter as usize];
        (spent <= limit).then_some(()).ok_or(Error::Budget { meter, limit, spent })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum LoadFault {
    /// Not a bundle: the first four bytes are not `LOFT`.
    NotBundle,
    /// A bundle of another format; `compiler` is the Loft commit that wrote it, when its JSON still names one.
    FormatMismatch { found: u32, expected: u32, compiler: Option<Box<str>> },
    /// The file ends before the header, JSON or a section it declares.
    Truncated { need: u64, have: u64 },
    /// Bytes after the last section: the file ends at `end`.
    Trailing { end: u64, have: u64 },
    /// The JSON does not decode as this format's `Bundle`.
    Json(Box<str>),
    /// A bundle that decodes but that the host refuses before any GPU object exists: an id out of range, a size over
    /// the device's limits, a library module Naga refuses. The message names which, and the fix.
    Invalid(Box<str>),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Internal {
    /// The recorder wrote a job the wire refuses.
    Wire(WireFault),
    /// A Rust panic, caught at the binding.
    Panic(Box<str>),
    /// The binding was called with the wrong arguments.
    Binding(Box<str>),
    /// A program the engine does not run yet (wave 1 decodes only; plan §9).
    Unbuilt(Program),
    /// Serialising a bundle or report failed.
    Json(Box<str>),
}

/// Where in the job the wire is wrong: the span, the element index in it, and what.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct WireFault {
    pub span: Span,
    pub at: u64,
    pub what: Fault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Span {
    Header,
    Nodes,
    Args,
    Lits,
    Rows,
    Blobs,
    Syms,
    SymAt,
    Sites,
    NodeSites,
    Decls,
    Roots,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Fault {
    Truncated,
    Magic,
    Version {
        found: u32,
    },
    Program {
        found: u32,
    },
    SpanCount {
        found: u32,
    },
    /// A span past the buffer, with a nonzero pad word or padding, or not a whole number of elements.
    Bounds,
    Utf8,
    /// A count that disagrees with another span (node sites per node, symbol offsets, decl roots), or bytes after the
    /// last span.
    Length,
    Op {
        code: u8,
    },
    Ty {
        code: u8,
    },
    Flags {
        found: u16,
    },
    /// An operand id not below its node, a root or decl reference out of range.
    Ref {
        found: u32,
    },
    /// A reference to a decl of the wrong kind, or a decl tag with no pinned layout.
    Tag {
        found: u32,
    },
    Arity {
        found: u32,
    },
    /// A literal outside `lits`, or not finite.
    Lit,
    Sym {
        found: u32,
    },
    /// A decl whose words or roots do not have its kind's layout.
    Layout,
    /// A Call or Loop of a decl with a root at or after the node itself: a cycle through the decl.
    Order {
        root: u32,
    },
    /// A binder read outside its loop or graph function (found by the canonical print, which walks scopes).
    Scope,
}

impl Error {
    pub fn code(&self) -> u8 {
        match self {
            Error::Internal(_) => 1,
            Error::Usage(_) => 2,
            Error::Load(_) => 3,
            Error::Scene { .. } => 4,
            Error::Environment(_) => 6,
            Error::Budget { .. } => 7,
        }
    }
    /// The verdict word src/cli/verdict.ts prints and the binding's payload carries.
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Internal(_) => "internal",
            Error::Usage(_) => "usage",
            Error::Load(_) => "load",
            Error::Scene { .. } => "scene",
            Error::Environment(_) => "environment",
            Error::Budget { .. } => "budget",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(m) | Error::Environment(m) => f.write_str(m),
            Error::Scene { site, fault } => {
                match fault {
                    SceneFault::Type { want, got } => write!(f, "given {got:?}; expected {want:?}")?,
                    SceneFault::Unsupported { what } => write!(f, "unsupported: {what}")?,
                    SceneFault::Infeasible { a, b } => write!(f, "infeasible: {a} conflicts with {b}")?,
                    SceneFault::Unsolved { fix } => write!(f, "unsolved. Fix: {fix}")?,
                    SceneFault::Unreadable => f.write_str("unreadable")?,
                    SceneFault::NotFinite { node } => write!(f, "not finite at node {node}")?,
                    SceneFault::Wgsl { line, msg } => write!(f, "WGSL line {line}: {msg}")?,
                }
                site.as_ref().map_or(Ok(()), |s| write!(f, " (at {s})"))
            }
            Error::Budget { meter, limit, spent } => write!(f, "{meter:?}: spent {spent} of {limit}"),
            Error::Load(fault) => write!(f, "bundle refused: {fault:?}"),
            Error::Internal(fault) => write!(f, "{fault:?}"),
        }
    }
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.col)
    }
}
