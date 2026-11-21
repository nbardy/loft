//! Encode source spans in generated WGSL until the orchestrator removes them.
use loft_ir::Span;
pub const OPEN: char = '\u{fdd0}';
pub const END: char = '\u{fdd1}';

pub(crate) fn expression(span: Span, body: String) -> String {
    format!(
        "{OPEN}{}:{}:{}{END}{body}{OPEN}{END}",
        span.file, span.lo, span.hi
    )
}
pub(crate) fn raw(span: Span, body: &str) -> String {
    expression(span, body.replace(OPEN, &format!("{OPEN}!{END}")))
}
