//! Lossless token tree. Byte spans partition the input, including EOF trivia.
//! WGSL islands are single leaves, so a Loft source edit never parses shader comments as Loft.
use loft_ir::{Code, Diag, Res, Span};
use logos::Logos;

#[derive(Logos, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    #[regex(r"[ \t]+")]
    Space,
    #[regex(r"\r?\n")]
    Newline,
    #[regex(r"#[^\r\n]*")]
    Comment,
    #[regex(r"[a-zA-Z_][a-zA-Z_0-9]*")]
    Word,
    #[regex(r"[0-9]+x[0-9]+")]
    Grid,
    #[regex(r"0x[0-9a-fA-F]+", priority = 5)]
    #[regex(r"[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?([a-zA-Z]+(\^-?[0-9]+)?)?")]
    Number,
    #[regex(r#""([^"\\\n]|\\.)*""#)]
    String,
    #[regex(r"[()\[\]{}.,;:=+*/%!@&|^'<>-]|::|->|\.\.|\|>|==|!=|<=|>=|&&|\|\||<<|>>")]
    Symbol,
    Raw,
    End,
}

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

fn raw_end(text: &str) -> Option<usize> {
    let b = text.as_bytes();
    let (mut i, mut depth) = (0, 0);
    while i < b.len() {
        match &b[i..] {
            [b'/', b'/', ..] => {
                i += b[i..]
                    .iter()
                    .position(|c| *c == b'\n')
                    .unwrap_or(b.len() - i)
            }
            [b'/', b'*', ..] => {
                let mut nested = 1;
                i += 2;
                while nested > 0 {
                    match b.get(i..)? {
                        [b'/', b'*', ..] => {
                            nested += 1;
                            i += 2;
                        }
                        [b'*', b'/', ..] => {
                            nested -= 1;
                            i += 2;
                        }
                        [] => return None,
                        _ => i += 1,
                    }
                }
            }
            [b'"', ..] => {
                i += 1;
                loop {
                    match b.get(i)? {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            [b'{', ..] => {
                depth += 1;
                i += 1;
            }
            [b'}', ..] => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => i += 1,
        }
    }
    None
}

impl Cst {
    pub fn parse(file: u16, text: &str) -> Res<Self> {
        let mut lexer = Kind::lexer(text);
        let span = |lo: usize, hi: usize| Span {
            file,
            lo: lo as u32,
            hi: hi as u32,
        };
        let mut pending = vec![];
        let mut tokens: Vec<Token> = vec![];
        let mut island = false;
        while let Some(token) = lexer.next() {
            let r = lexer.span();
            let mut s = span(r.start, r.end);
            let mut k = token.map_err(|()| {
                Diag::new(
                    Code::E001,
                    s,
                    "unrecognised token",
                    "use an E+ identifier, literal or operator",
                )
            })?;
            let trivia = match k {
                Kind::Space => Some(Trivia::Whitespace),
                Kind::Newline => Some(Trivia::Newline),
                Kind::Comment => Some(Trivia::Comment),
                _ => None,
            };
            if let Some(t) = trivia {
                pending.push((t, s));
                if t == Trivia::Comment
                    && !pending.iter().any(|(t, _)| *t == Trivia::Newline)
                    && let Some(prev) = tokens.last_mut()
                {
                    prev.trailing.append(&mut pending);
                }
                continue;
            }
            if k == Kind::Number
                && !lexer.slice().starts_with("0x")
                && lexer
                    .slice()
                    .bytes()
                    .any(|b| b.is_ascii_alphabetic() && !matches!(b, b'e' | b'E'))
            {
                // Consume a whole unit atom: the s of sin after a product is an identifier.
                while let Some(rest) = lexer.remainder().strip_prefix(['*', '/']) {
                    let len = rest
                        .bytes()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
                        .count();
                    if !matches!(
                        &rest[..len],
                        "m" | "mm" | "s" | "kg" | "rad" | "deg" | "N" | "Hz" | "J"
                    ) {
                        break;
                    }
                    let mut n = 1 + len;
                    if let Some(pow) = rest[len..].strip_prefix('^') {
                        let digits = pow.strip_prefix('-').unwrap_or(pow);
                        let count = digits.bytes().take_while(u8::is_ascii_digit).count();
                        if count > 0 {
                            n += 1 + pow.len() - digits.len() + count;
                        }
                    }
                    lexer.bump(n);
                }
                s.hi = lexer.span().end as u32;
            }
            if island && lexer.slice() == "{" {
                let end = raw_end(&text[r.start..]).ok_or_else(|| {
                    Diag::new(
                        Code::E001,
                        s,
                        "unclosed WGSL island",
                        "close the shader block, string or comment",
                    )
                })?;
                lexer.bump(end - 1);
                s.hi = (r.start + end) as u32;
                k = Kind::Raw;
                island = false;
            } else if lexer.slice() == "wgsl" {
                island = true;
            }
            tokens.push(Token {
                kind: k,
                span: s,
                leading: std::mem::take(&mut pending),
                trailing: vec![],
            });
        }
        tokens.push(Token {
            leading: pending,
            trailing: vec![],
            kind: Kind::End,
            span: span(text.len(), text.len()),
        });
        let mut roots = vec![];
        let mut stack = vec![];
        for t in tokens {
            let lexeme = t.text(text);
            match lexeme {
                "(" | "[" | "{" => stack.push((t, std::mem::take(&mut roots))),
                ")" | "]" | "}" => {
                    let Some((open, mut parents)) = stack.pop() else {
                        return Err(Diag::new(
                            Code::E001,
                            t.span,
                            "unmatched closing delimiter",
                            "remove the extra delimiter",
                        ));
                    };
                    if !matches!(
                        (open.text(text), lexeme),
                        ("(", ")") | ("[", "]") | ("{", "}")
                    ) {
                        return Err(Diag::new(
                            Code::E001,
                            t.span,
                            "mismatched delimiter",
                            "close the innermost group first",
                        )
                        .note(open.span, "opened here"));
                    }
                    parents.push(Node::Group {
                        open,
                        children: roots,
                        close: t,
                    });
                    roots = parents;
                }
                _ => roots.push(Node::Leaf(t)),
            }
        }
        if let Some((open, _)) = stack.last() {
            return Err(Diag::new(
                Code::E001,
                open.span,
                "unclosed group",
                "add its matching closing delimiter",
            ));
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

    pub fn write(&self, source: &str) -> String {
        let mut out = String::new();
        for t in self.tokens() {
            for (_, s) in &t.leading {
                out.push_str(&source[s.lo as usize..s.hi as usize]);
            }
            out.push_str(t.text(source));
            for (_, s) in &t.trailing {
                out.push_str(&source[s.lo as usize..s.hi as usize]);
            }
        }
        out
    }
}
impl Token {
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.span.lo as usize..self.span.hi as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_comment_attachment_and_raw_braces() {
        let src = "# leading λ\r\nfn f(x: f) = wgsl (x: f) -> f { /* } /* { */ } */ if(x>0){return x;} // }\n return 0.; } # trailing\r\n";
        let tree = Cst::parse(0, src).unwrap();
        assert_eq!(tree.write(src), src);
        let raw = tree
            .tokens()
            .into_iter()
            .find(|t| t.kind == Kind::Raw)
            .unwrap();
        assert!(raw.trailing.iter().any(|(k, _)| *k == Trivia::Comment));
    }
}

#[cfg(test)]
mod corpus {
    use super::Cst;
    use std::{fs, path::Path};

    #[test]
    fn all_authored_files_replay_verbatim() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for dir in ["lib", "engines", "agent_notes/2026-11-20_meta/v1_syntax"] {
            for entry in fs::read_dir(root.join(dir)).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|ext| ext == "loft") {
                    let source = fs::read_to_string(&path).unwrap();
                    assert_eq!(
                        Cst::parse(0, &source).unwrap().write(&source),
                        source,
                        "{}",
                        path.display()
                    );
                }
            }
        }
    }

    #[test]
    fn unit_powers_and_reciprocals_are_accepted_in_type_position() {
        crate::parse(0, "fn convert(x: 1/kg, y: m^2/s^2) -> J = x * y", &mut 0).unwrap();
        crate::parse(0, "fn divide(s: f) -> f = 1e2/s", &mut 0).unwrap();
    }
}
