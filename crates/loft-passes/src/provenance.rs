//! Carry an expression's author span through WGSL string composition. Markers are
//! removed before Naga sees its input, so source edits cannot change module text.
use loft_ir::{Sources, Span};
use crate::program::ExpressionOrigin;
const OPEN: char = '\u{fdd0}';
const END: char = '\u{fdd1}';

pub(crate) fn expression(span: Span, body: String) -> String {
    format!(
        "{OPEN}{}:{}:{}{END}{body}{OPEN}{END}",
        span.file, span.lo, span.hi
    )
}
pub(crate) fn raw(span: Span, body: &str) -> String {
    expression(span, body.replace(OPEN, &format!("{OPEN}!{END}")))
}
pub(crate) struct Map {
    pub shader: String,
    changes: Vec<(usize, Option<Span>)>,
}
impl Map {
    pub fn new(text: String) -> Self {
        let mut shader = String::with_capacity(text.len());
        let mut changes = vec![];
        let mut active = vec![];
        let mut input = text.as_str();
        while let Some((code, next)) = input.split_once(OPEN) {
            shader.push_str(code);
            let (tag, rest) = next.split_once(END).expect("emitter closes markers");
            match tag {
                "" => {
                    active.pop();
                }
                "!" => {
                    shader.push(OPEN);
                    input = rest;
                    continue;
                }
                s => {
                    let mut parts = s.split(':');
                    let file = parts.next().unwrap().parse().unwrap();
                    let lo = parts.next().unwrap().parse().unwrap();
                    let hi = parts.next().unwrap().parse().unwrap();
                    active.push(Span { file, lo, hi });
                }
            }
            changes.push((shader.len(), active.last().copied()));
            input = rest;
        }
        shader.push_str(input);
        Self { shader, changes }
    }
    pub fn origin(&self, span: naga::Span) -> Option<Span> {
        let offset = span.to_range()?.start;
        let pos = self.changes.partition_point(|(at, _)| *at <= offset);
        self.changes
            .get(pos.checked_sub(1)?)?
            .1
            // An embedded recipe has offsets for its own parser but no file in Sources.
            .filter(|s| s.file != Span::NONE.file)
    }
    pub fn expressions(&self, module: &naga::Module, src: &Sources) -> Vec<ExpressionOrigin> {
        let line_starts: Vec<Vec<usize>> = src
            .files
            .iter()
            .map(|(_, s)| {
                std::iter::once(0)
                    .chain(s.match_indices('\n').map(|(i, _)| i + 1))
                    .collect()
            })
            .collect();
        module
            .functions
            .iter()
            .flat_map(|(f, body)| {
                body.expressions.iter().filter_map({
                    let lines = &line_starts;
                    move |(id, _)| {
                        let s = self.origin(body.expressions.get_span(id))?;
                        let start = s.lo as usize;
                        let path = &src.files[s.file as usize].0;
                        let starts = &lines[s.file as usize];
                        let line = starts.partition_point(|i| *i <= start);
                        Some(ExpressionOrigin {
                            function: f.index(),
                            expression: id.index(),
                            span: (path.clone(), line, 1 + start - starts[line - 1]),
                        })
                    }
                })
            })
            .collect()
    }
}
