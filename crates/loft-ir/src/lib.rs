//! Layer 1 of the meta engine (plan 2026-11-20 §1): the source tree and its types, diagnostics that name the fix,
//! three-valued verdicts, and the `Program` the runner executes. No pass logic lives here.

pub mod ast;
pub mod program;

use std::fmt;

/// A byte range in one source file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub file: u16,
    pub lo: u32,
    pub hi: u32,
}

impl Span {
    /// No source location: a compiler-internal diagnostic (generated code, the environment).
    pub const NONE: Span = Span { file: u16::MAX, lo: 0, hi: 0 };

    pub fn to(self, end: Span) -> Span {
        Span { file: self.file, lo: self.lo, hi: end.hi.max(self.lo) }
    }
}

/// Every file of one program: the library prelude, then the scene.
#[derive(Default)]
pub struct Sources {
    pub files: Vec<(String, String)>,
}

impl Sources {
    pub fn add(&mut self, path: impl Into<String>, text: impl Into<String>) -> u16 {
        self.files.push((path.into(), text.into()));
        u16::try_from(self.files.len() - 1).expect("fewer than 65,536 source files")
    }

    /// `path:line:col`, the line's text, and the column range of the span on that line.
    pub fn locate(&self, s: Span) -> (String, usize, usize, &str, usize) {
        let (path, text) = &self.files[s.file as usize];
        let lo = (s.lo as usize).min(text.len());
        let line_start = text[..lo].rfind('\n').map_or(0, |i| i + 1);
        let line_end = text[lo..].find('\n').map_or(text.len(), |i| lo + i);
        let line = text[..lo].matches('\n').count() + 1;
        let col = lo - line_start + 1;
        let width = (s.hi as usize).min(line_end).saturating_sub(lo).max(1);
        (path.clone(), line, col, &text[line_start..line_end], width)
    }
}

/// The error codes. Each names a class of mistake; the message and `fix` name the instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    /// Syntax.
    E001,
    /// Unknown name, or a name used as the wrong thing.
    E002,
    /// Type mismatch outside the affine/space rules.
    E003,
    /// Space mismatch or an affine rule broken (`Pt + Pt`, a transform applied in the wrong space).
    E104,
    /// A sphere-traced field without a proven Lipschitz bound.
    E211,
    /// A traced field whose surface has no finite derived bound.
    E212,
    /// A continuous type parameterises kind identity.
    E401,
    /// A kind's discrete variant budget is exceeded.
    E402,
    /// Stage-0 code (`emit`, `comptime for`) outside a `comptime fn`.
    E403,
    /// Kind code (stage 1) reads a stage-0 name.
    E404,
    /// Valid language that Phase 0 does not lower yet: a named refusal, never a fallback.
    E405,
    /// A schedule that cannot be lowered.
    E501,
    /// A row that cannot be emitted (missing or unknown field, wrong value).
    E601,
    /// Evaluation failed on the CPU (stage 0 or the twin).
    E701,
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// One diagnostic: where, what, what led there, and the one-sentence fix.
#[derive(Clone, Debug)]
pub struct Diag {
    pub code: Code,
    pub span: Span,
    pub msg: String,
    pub notes: Vec<(Span, String)>,
    pub fix: String,
}

impl Diag {
    pub fn new(code: Code, span: Span, msg: impl Into<String>, fix: impl Into<String>) -> Diag {
        Diag { code, span, msg: msg.into(), notes: vec![], fix: fix.into() }
    }
    pub fn note(mut self, span: Span, text: impl Into<String>) -> Diag {
        self.notes.push((span, text.into()));
        self
    }

    /// The rustc-style rendering: the author's own lines, never generated code.
    pub fn render(&self, src: &Sources) -> String {
        let mut out = format!("error[{}]: {}\n", self.code, self.msg);
        let mut show = |s: Span, label: &str, mark: char| {
            if s.file as usize >= src.files.len() {
                return;
            }
            let (path, line, col, text, width) = src.locate(s);
            out += &format!("  --> {path}:{line}:{col}\n   |\n{line:>3} | {text}\n   | ");
            out += &" ".repeat(col - 1);
            out += &mark.to_string().repeat(width);
            if !label.is_empty() {
                out += &format!(" {label}");
            }
            out += "\n";
        };
        show(self.span, "", '^');
        for (s, n) in &self.notes {
            show(*s, n, '-');
        }
        out += &format!("   = fix: {}\n", self.fix);
        out
    }
}

pub type Res<T> = Result<T, Diag>;

/// What any check reports (plan §2.5): gates pass only on `Held`.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Held { margin: f64 },
    Missed { by: f64 },
    Unknown { reason: String },
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::Held { margin } => write!(f, "Held(margin {margin:.3e})"),
            Verdict::Missed { by } => write!(f, "Missed(by {by:.3e})"),
            Verdict::Unknown { reason } => write!(f, "Unknown({reason})"),
        }
    }
}
