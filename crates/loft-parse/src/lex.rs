//! Flat tokens with byte spans into the original Loft source.
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
    #[regex(r"[0-9]+x[0-9]+(?:x[0-9]+)?")]
    Grid,
    #[regex(r"0x[0-9a-fA-F]+", priority = 5)]
    #[regex(r"[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?([a-zA-Z]+(\^-?[0-9]+)?)?")]
    Number,
    #[regex(r#""([^"\\\n]|\\.)*""#)]
    String,
    #[regex(r"[()\[\]{}.,;:=+*/%!@&|^'<>-]|::|->|=>|\.\.|\|>|==|!=|<=|>=|&&|\|\||<<|>>")]
    Symbol,
    Raw,
    End,
}

#[derive(Clone, Copy)]
pub(crate) struct Lex<'a> {
    pub(crate) kind: Kind,
    pub(crate) text: &'a str,
    pub(crate) span: Span,
}

fn continued(prev: &str, next: &str, depth: usize) -> bool {
    depth > 0
        || matches!(
            prev,
            "+" | "-"
                | "*"
                | "/"
                | "%"
                | "=="
                | "!="
                | "<"
                | "<="
                | ">"
                | ">="
                | "&&"
                | "||"
                | ","
                | "="
                | "|>"
        )
        // A line that starts with a binary operator cannot start a statement, so it continues the last one:
        // `ggx(..)\n  + coat(..)`. Models stack lobes this way (4 of 16 fresh crab programs). `-` is excluded
        // because it can begin a unary expression.
        || matches!(next, "|>" | "else" | "+" | "*" | "/" | "%" | "==" | "!=" | "<=" | ">=" | "&&" | "||")
}

pub(crate) fn lex(file: u16, source: &str) -> Res<Vec<Lex<'_>>> {
    let mut tokens: Vec<Lex<'_>> = Vec::new();
    let mut newline = None;
    let mut depth = 0;
    let mut prev = "";
    scan(file, source, |token| {
        match token.kind {
            Kind::Newline => { newline.get_or_insert(token.span); return; }
            Kind::Space | Kind::Comment => return,
            Kind::Word | Kind::Grid | Kind::Number | Kind::String | Kind::Symbol | Kind::Raw | Kind::End => {}
        }
        let import_star = prev == "*" && tokens.iter().rev().nth(1).is_some_and(|t| t.text == "::");
        if let Some(span) = newline.take()
            && (import_star || !continued(prev, token.text, depth))
        {
            tokens.push(Lex { text: ";", kind: Kind::Newline, span });
        }
        match token.text {
            "(" | "[" => depth += 1,
            ")" | "]" => depth -= 1,
            _ => {}
        }
        prev = token.text;
        tokens.push(token);
    })?;
    Ok(tokens)
}

/// Feed the parser and source tools the same lexemes and delimiter diagnostics.
pub(crate) fn scan<'a>(file: u16, source: &'a str, mut push: impl FnMut(Lex<'a>)) -> Res<()> {
    let span = |lo: usize, hi: usize| Span { file, lo: lo as u32, hi: hi as u32 };
    let mut lexer = Kind::lexer(source);
    let mut groups = Vec::new();
    let mut island = false;
    while let Some(token) = lexer.next() {
        let start = lexer.span().start;
        let mut site = span(start, lexer.span().end);
        let mut kind = token.map_err(|()| {
            Diag::new(Code::E001, site, "unrecognised token", "use an E+ identifier, literal or operator")
        })?;
        if kind == Kind::Number && !lexer.slice().starts_with("0x")
            && lexer.slice().bytes().any(|b| b.is_ascii_alphabetic() && !matches!(b, b'e' | b'E'))
        {
            while let Some(rest) = lexer.remainder().strip_prefix(['*', '/']) {
                let len = rest.bytes().take_while(|c| c.is_ascii_alphanumeric() || *c == b'_').count();
                // Consume only a whole unit atom, so a divisor such as /sin remains separate.
                if !unit(&rest[..len]) { break; }
                let mut end = len;
                if let Some(power) = rest[end..].strip_prefix('^') {
                    let digits = power.strip_prefix('-').unwrap_or(power);
                    let count = digits.bytes().take_while(u8::is_ascii_digit).count();
                    if count > 0 { end += 1 + power.len() - digits.len() + count; }
                }
                lexer.bump(1 + end);
            }
            site.hi = lexer.span().end as u32;
            if lexer.remainder().as_bytes().first().is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
                return Err(Diag::new(Code::E001, site, "number or unit touches an identifier",
                    "separate the number or unit from the identifier"));
            }
        }
        if island && lexer.slice() == "{" {
            let end = raw_end(&source[start..]).ok_or_else(|| {
                Diag::new(Code::E001, site, "unclosed WGSL island", "close the shader block, string or comment")
            })?;
            lexer.bump(end - 1);
            site.hi = (start + end) as u32;
            kind = Kind::Raw;
            island = false;
        } else if lexer.slice() == "wgsl" {
            island = true;
        }
        let text = lexer.slice();
        match text {
            "(" | "[" | "{" => groups.push((text, site)),
            ")" | "]" | "}" => {
                let Some((open, opened_at)) = groups.pop() else {
                    return Err(Diag::new(Code::E001, site, "unmatched closing delimiter", "remove the extra delimiter"));
                };
                if !matches!((open, text), ("(", ")") | ("[", "]") | ("{", "}")) {
                    return Err(Diag::new(Code::E001, site, "mismatched delimiter", "close the innermost group first")
                        .note(opened_at, "opened here"));
                }
            }
            _ => {}
        }
        push(Lex { kind, text, span: site });
    }
    if let Some((_, open)) = groups.pop() {
        return Err(Diag::new(Code::E001, open, "unclosed group", "add its matching closing delimiter"));
    }
    push(Lex { kind: Kind::End, text: "", span: span(source.len(), source.len()) });
    Ok(())
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

pub const UNIT_SYMBOLS: &[&str] = &["m", "mm", "rad", "deg", "kg", "s", "N", "J", "Hz"];

pub(crate) fn unit(s: &str) -> bool {
    UNIT_SYMBOLS.contains(&s)
}
