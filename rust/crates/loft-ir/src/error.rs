//! The one engine error (plan §1.8). Exit codes keep today's numbers (src/cli/verdict.ts): internal 1, usage 2,
//! load 3, scene 4, environment 6, budget 7. Exit 5 (`check`) is a verdict, not an error.
use crate::graph::{Leaf, Program, Site, Ty};
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
    NotFinite {
        at: Leaf,
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
    SymEnds,
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
    /// A span outside the buffer, overlapping the header, or not a whole number of aligned elements.
    Bounds,
    Utf8,
    /// A count that disagrees with another span (node sites per node, sym ends past the text).
    Length,
    Op {
        code: u8,
    },
    Ty {
        code: u8,
    },
    Flags {
        found: u8,
    },
    /// An operand id not below its node, a root or decl reference out of range.
    Ref {
        found: u32,
    },
    /// A reference to a decl of the wrong kind.
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
    /// A decl whose roots do not have its kind's layout.
    Layout,
    /// A Call or Loop of a decl with a root at or after the node itself: a cycle through the decl.
    Order {
        root: u32,
    },
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
                    SceneFault::NotFinite { at } => write!(f, "not finite at {at:?}")?,
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
