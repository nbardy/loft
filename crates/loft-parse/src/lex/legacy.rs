//! Grouped source tokens used by the CLI's catalog and lint commands.
pub use crate::lex::Kind;
use crate::lex::scan;
use loft_ir::{Res, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trivia {
    Whitespace,
    Newline,
    Comment,
}
#[derive(Clone, Debug)]
pub struct Token {
    pub leading: Vec<(Trivia, Span)>,
    pub kind: Kind,
    pub span: Span,
    pub trailing: Vec<(Trivia, Span)>,
}
#[derive(Clone, Debug)]
pub enum Node {
    Leaf(Token),
    Group {
        open: Token,
        children: Vec<Node>,
        close: Token,
    },
}
#[derive(Clone, Debug)]
pub struct Cst {
    pub roots: Vec<Node>,
}

impl Cst {
    pub fn parse(file: u16, source: &str) -> Res<Self> {
        let mut pending = Vec::new();
        let mut tokens: Vec<Token> = Vec::new();
        scan(file, source, |lex| {
            let trivia = match lex.kind {
                Kind::Space => Some(Trivia::Whitespace),
                Kind::Newline => Some(Trivia::Newline),
                Kind::Comment => Some(Trivia::Comment),
                Kind::Word | Kind::Grid | Kind::Number | Kind::String | Kind::Symbol | Kind::Raw | Kind::End => None,
            };
            if let Some(trivia) = trivia {
                pending.push((trivia, lex.span));
                if trivia == Trivia::Comment
                    && !pending.iter().any(|(kind, _)| *kind == Trivia::Newline)
                    && let Some(prev) = tokens.last_mut()
                {
                    prev.trailing.append(&mut pending);
                }
            } else {
                tokens.push(Token {
                    kind: lex.kind,
                    span: lex.span,
                    leading: std::mem::take(&mut pending),
                    trailing: Vec::new(),
                });
            }
        })?;
        let mut roots = Vec::new();
        let mut stack = Vec::new();
        for token in tokens {
            match token.text(source) {
                "(" | "[" | "{" => stack.push((token, std::mem::take(&mut roots))),
                ")" | "]" | "}" => {
                    let (open, mut parents) = stack.pop().expect("scanner paired all delimiters");
                    parents.push(Node::Group { open, children: roots, close: token });
                    roots = parents;
                }
                _ => roots.push(Node::Leaf(token)),
            }
        }
        Ok(Self { roots })
    }

    pub fn tokens(&self) -> Vec<&Token> {
        fn walk<'a>(nodes: &'a [Node], out: &mut Vec<&'a Token>) {
            for n in nodes {
                match n {
                    Node::Leaf(t) => out.push(t),
                    Node::Group {
                        open,
                        children,
                        close,
                    } => {
                        out.push(open);
                        walk(children, out);
                        out.push(close);
                    }
                }
            }
        }
        let mut out = vec![];
        walk(&self.roots, &mut out);
        out
    }

}
impl Token {
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.span.lo as usize..self.span.hi as usize]
    }
}
