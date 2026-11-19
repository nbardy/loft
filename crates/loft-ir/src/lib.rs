//! Shared execution declarations, source locations, diagnostics and verdicts.
mod bundle;
pub use bundle::*;


use std::fmt;

/// A byte range in one source file.
#[derive(serde::Serialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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

/// The diagnostic codes (`docs/spec/LOFT_V1_SYNTAX.md` §7). Each names a class of mistake; the message and `fix` name
/// the instance. `W` codes are warnings: reported, never fatal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    /// Syntax, or an unknown unit suffix.
    E001,
    /// Type mismatch outside the affine and space rules.
    E003,
    /// Unknown name, or a name used as the wrong thing.
    E011,
    /// `p3 + p3`, or another affine rule broken.
    E101,
    /// Space mismatch, or a map's space used outside it.
    E104,
    E105,
    /// A map misused: bare map name, `.cell` without a mirror, a map of something other than a query point.
    E106,
    /// A bare kind where a function reads a table (`fish: Fish` for `fishes: [Fish]`), or a row id of another kind.
    E108,
    /// A traced field or surface with no finite derived bound.
    E201,
    /// A numeric Lipschitz bound written in a scene.
    E210,
    /// A sphere-traced field whose L is unbounded (a divisor that can reach zero).
    E211,
    /// A drawn kind without a required query.
    E212,
    /// `diff` of an expression that is not differentiable (a `C0` kink).
    E307,
    /// A scan frame that degenerates (zero tangent).
    E308,
    /// A continuous type parameterises kind identity.
    E401,
    /// A kind's discrete variant budget is exceeded.
    E402,
    /// A value read at the wrong stage: `time` in build code, or kind code reading a build name.
    E403,
    /// Build code outside build: a module-level statement, `emit`, or a loop that is not `build for`.
    E404,
    /// Valid language that this phase does not lower yet: a named refusal, never a fallback.
    E405,
    /// A field write outside its declared write set, or a write to a table.
    E501,
    /// Two laws write the same field.
    E502,
    /// A solver constraint would race.
    E503,
    E504,
    E505,
    E110,
    /// A schedule or default-engine configuration that cannot be lowered.
    E520,
    /// A row that cannot be emitted: missing or unknown column, a value outside its range.
    E602,
    /// Evaluation failed on the CPU (build or the twin).
    E701,
    /// A mirrored kind emitted in mirrored pairs (the mirror already makes the pair).
    W410,
}

impl Code {
    pub fn is_warning(self) -> bool {
        matches!(self, Code::W410)
    }
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
    /// Further diagnostics reported with this one (parse errors after recovery).
    pub also: Vec<Diag>,
}

impl Diag {
    pub fn new(code: Code, span: Span, msg: impl Into<String>, fix: impl Into<String>) -> Diag {
        Diag { code, span, msg: msg.into(), notes: vec![], fix: fix.into(), also: vec![] }
    }
    pub fn note(mut self, span: Span, text: impl Into<String>) -> Diag {
        self.notes.push((span, text.into()));
        self
    }

    /// The rustc-style rendering: the author's own lines, never generated code.
    pub fn render(&self, src: &Sources) -> String {
        let kind = if self.code.is_warning() { "warning" } else { "error" };
        let mut out = format!("{kind}[{}]: {}\n", self.code, self.msg);
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
        for d in &self.also {
            out += "\n";
            out += &d.render(src);
        }
        out
    }
}

pub type Res<T> = Result<T, Diag>;

/// What any check reports (§2.5): gates pass only on `Held`.
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
