//! Lexer and recursive-descent parser for the Phase 0 subset of the WGSL superset (draft §1): `space`, `unit`,
//! `const`/`comptime const`, `fn`/`comptime fn` with `@attrs`, `kind` with comptime params, `row` and `query`,
//! `schedule`; statements `let`/`var`, assignment, `map ... |> ...`, `return`, `if`, `for i in a..b`, `emit`.

use loft_ir::ast::*;
use loft_ir::{Code, Diag, Res, Span};

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    /// `'name`
    SpaceVar(String),
    Num { value: f64, suffix: NumSuffix, text: String, float: bool },
    Punct(&'static str),
    Eof,
}

const PUNCT: [&str; 38] = [
    "|>", "->", "..", "<=", ">=", "==", "!=", "&&", "||", "<<", ">>", "+=", "-=", "*=", "/=", "{", "}", "(", ")", "[",
    "]", "<", ">", ",", ";", ":", ".", "=", "+", "-", "*", "/", "%", "!", "@", "&", "|", "^",
];

fn lex(file: u16, text: &str) -> Res<Vec<(Tok, Span)>> {
    let b = text.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    let span = |lo: usize, hi: usize| Span { file, lo: lo as u32, hi: hi as u32 };
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            let end = text[i + 2..].find("*/").ok_or_else(|| {
                Diag::new(Code::E001, span(i, i + 2), "unterminated block comment", "close it with `*/`")
            })?;
            i += end + 4;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((Tok::Ident(text[s..i].into()), span(s, i)));
        } else if c == b'\'' {
            let s = i;
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((Tok::SpaceVar(text[s + 1..i].into()), span(s, i)));
        } else if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            let s = i;
            let mut float = false;
            let value = if b[i..].starts_with(b"0x") {
                i += 2;
                while i < b.len() && b[i].is_ascii_hexdigit() {
                    i += 1;
                }
                u64::from_str_radix(&text[s + 2..i], 16).map_err(|_| {
                    Diag::new(Code::E001, span(s, i), "bad hex literal", "write hex digits after 0x")
                })? as f64
            } else {
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                // `0..6`: a range, not a decimal point.
                if i < b.len() && b[i] == b'.' && b.get(i + 1) != Some(&b'.') {
                    float = true;
                    i += 1;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                if i < b.len()
                    && (b[i] == b'e' || b[i] == b'E')
                    && b.get(i + 1).is_some_and(|d| d.is_ascii_digit() || *d == b'-' || *d == b'+')
                {
                    float = true;
                    i += 2;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                text[s..i].parse::<f64>().map_err(|_| {
                    Diag::new(Code::E001, span(s, i), "bad number", "write a decimal number like 0.5 or 2")
                })?
            };
            let num_end = i;
            while i < b.len() && b[i].is_ascii_alphabetic() {
                i += 1;
            }
            let suffix = match &text[num_end..i] {
                "" => NumSuffix::None,
                "f" => NumSuffix::F,
                "u" => NumSuffix::U,
                "i" => NumSuffix::I,
                "m" => NumSuffix::M,
                "rad" => NumSuffix::Rad,
                "deg" => NumSuffix::Deg,
                "s" => NumSuffix::Sec,
                "kg" => NumSuffix::Kg,
                other => {
                    return Err(Diag::new(
                        Code::E001,
                        span(num_end, i),
                        format!("unknown literal suffix `{other}`"),
                        "use f, u, i, or a unit: m, rad, deg, s, kg",
                    ));
                }
            };
            let float = float || !matches!(suffix, NumSuffix::None | NumSuffix::U | NumSuffix::I);
            out.push((Tok::Num { value, suffix, text: text[s..num_end].into(), float }, span(s, i)));
        } else {
            let p = PUNCT.iter().find(|p| b[i..].starts_with(p.as_bytes())).ok_or_else(|| {
                Diag::new(Code::E001, span(i, i + 1), format!("unexpected character `{}`", c as char), "remove it")
            })?;
            out.push((Tok::Punct(p), span(i, i + p.len())));
            i += p.len();
        }
    }
    out.push((Tok::Eof, span(b.len(), b.len())));
    Ok(out)
}

pub struct Parser<'a> {
    toks: Vec<(Tok, Span)>,
    at: usize,
    next_id: &'a mut u32,
    diffs: Vec<(Ident, Ident)>,
}

/// Parses one file; expression ids continue from `next_id` so a whole program shares one id space.
pub fn parse(file: u16, text: &str, next_id: &mut u32) -> Res<Module> {
    let mut p = Parser { toks: lex(file, text)?, at: 0, next_id, diffs: vec![] };
    let mut m = Module::default();
    while p.peek() != &Tok::Eof {
        m.items.push(p.item()?);
    }
    m.diffs = p.diffs;
    Ok(m)
}

const GEO: [(&str, Geo); 4] = [("Pt", Geo::Pt), ("Vec", Geo::Vec), ("Dir", Geo::Dir), ("Norm", Geo::Norm)];

impl Parser<'_> {
    fn peek(&self) -> &Tok {
        &self.toks[self.at].0
    }
    fn peek2(&self) -> &Tok {
        &self.toks[(self.at + 1).min(self.toks.len() - 1)].0
    }
    fn span(&self) -> Span {
        self.toks[self.at].1
    }
    fn prev_span(&self) -> Span {
        self.toks[self.at.saturating_sub(1)].1
    }
    fn bump(&mut self) -> (Tok, Span) {
        let t = self.toks[self.at].clone();
        self.at = (self.at + 1).min(self.toks.len() - 1);
        t
    }
    fn is(&self, p: &str) -> bool {
        matches!(self.peek(), Tok::Punct(q) if *q == p)
    }
    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Tok::Ident(q) if q == w)
    }
    fn eat(&mut self, p: &str) -> bool {
        let hit = self.is(p);
        if hit {
            self.bump();
        }
        hit
    }
    fn eat_word(&mut self, w: &str) -> bool {
        let hit = self.is_word(w);
        if hit {
            self.bump();
        }
        hit
    }
    fn err<T>(&self, want: &str) -> Res<T> {
        let found = match self.peek() {
            Tok::Ident(s) => format!("`{s}`"),
            Tok::SpaceVar(s) => format!("`'{s}`"),
            Tok::Num { text, .. } => format!("number `{text}`"),
            Tok::Punct(p) => format!("`{p}`"),
            Tok::Eof => "end of file".into(),
        };
        Err(Diag::new(Code::E001, self.span(), format!("expected {want}, found {found}"), format!("write {want} here")))
    }
    fn expect(&mut self, p: &str) -> Res<Span> {
        if self.is(p) { Ok(self.bump().1) } else { self.err(&format!("`{p}`")) }
    }
    fn ident(&mut self) -> Res<Ident> {
        match self.peek().clone() {
            Tok::Ident(name) => {
                let span = self.bump().1;
                Ok(Ident { name, span })
            }
            Tok::Punct(_) | Tok::SpaceVar(_) | Tok::Num { .. } | Tok::Eof => self.err("a name"),
        }
    }
    fn id(&mut self) -> u32 {
        *self.next_id += 1;
        *self.next_id - 1
    }

    fn item(&mut self) -> Res<Item> {
        let mut attrs = vec![];
        while self.eat("@") {
            let name = self.ident()?;
            let args = if self.is("(") { self.args()?.into_iter().map(|a| a.value).collect() } else { vec![] };
            attrs.push(Attr { name, args });
        }
        let comptime = self.eat_word("comptime");
        if self.eat_word("fn") {
            return Ok(Item::Fn(self.fn_rest(comptime, attrs)?));
        }
        if !attrs.is_empty() {
            return self.err("`fn` after an attribute");
        }
        if self.eat_word("const") {
            let name = self.ident()?;
            let ty = if self.eat(":") { Some(self.ty()?) } else { None };
            self.expect("=")?;
            let value = self.expr()?;
            self.expect(";")?;
            return Ok(Item::Const(ConstDecl { comptime, name, ty, value }));
        }
        if comptime {
            return self.err("`fn` or `const` after `comptime`");
        }
        if self.eat_word("space") {
            let mut names = vec![self.ident()?];
            while self.eat(",") {
                names.push(self.ident()?);
            }
            self.expect(";")?;
            return Ok(Item::Space(names));
        }
        if self.eat_word("unit") {
            let name = self.ident()?;
            self.expect(";")?;
            return Ok(Item::Unit(name));
        }
        if self.eat_word("kind") {
            return Ok(Item::Kind(self.kind()?));
        }
        if self.eat_word("schedule") {
            return Ok(Item::Schedule(self.schedule()?));
        }
        if self.eat_word("scan") {
            return Ok(Item::Scan(self.scan()?));
        }
        self.err("an item (`space`, `unit`, `const`, `fn`, `kind`, `scan`, `schedule`)")
    }

    /// `scan s(t: f32 in lo..hi, steps: N) @schedule(bake_lut) -> Table[T] { init: e; step(prev, t_i): e; }`
    fn scan(&mut self) -> Res<ScanDecl> {
        let name = self.ident()?;
        self.expect("(")?;
        let var = self.ident()?;
        self.expect(":")?;
        let var_ty = self.ty()?;
        self.expect_word("in")?;
        let lo = self.expr_bp(7)?;
        self.expect("..")?;
        let hi = self.expr_bp(7)?;
        self.expect(",")?;
        self.expect_word("steps")?;
        self.expect(":")?;
        let steps = self.expr()?;
        self.expect(")")?;
        self.expect("@")?;
        self.expect_word("schedule")?;
        self.expect("(")?;
        let mode = self.ident()?;
        if mode.name != "bake_lut" {
            return Err(Diag::new(Code::E405, mode.span, "Phase 1 bakes scans at load only", "write `@schedule(bake_lut)`; per-frame scans arrive with motion"));
        }
        self.expect(")")?;
        self.expect("->")?;
        let table = self.ident()?;
        if table.name != "Table" {
            return self.err("`Table[T]`");
        }
        self.expect("[")?;
        let elem = self.ty()?;
        self.expect("]")?;
        self.expect("{")?;
        self.expect_word("init")?;
        self.expect(":")?;
        let init = self.expr()?;
        self.expect(";")?;
        self.expect_word("step")?;
        self.expect("(")?;
        let prev = self.ident()?;
        self.expect(",")?;
        let at = self.ident()?;
        self.expect(")")?;
        self.expect(":")?;
        let step = self.expr()?;
        self.expect(";")?;
        self.expect("}")?;
        Ok(ScanDecl { name, var, var_ty, lo, hi, steps, elem, init, prev, at, step })
    }

    fn fn_rest(&mut self, comptime: bool, attrs: Vec<Attr>) -> Res<FnDecl> {
        let name = self.ident()?;
        self.expect("(")?;
        let mut params = vec![];
        while !self.is(")") {
            let pname = self.ident()?;
            let ty = if pname.name == "self" && !self.is(":") {
                None
            } else {
                self.expect(":")?;
                Some(self.ty()?)
            };
            params.push(Param { name: pname, ty });
            if !self.eat(",") {
                break;
            }
        }
        self.expect(")")?;
        let ret = if self.eat("->") { Some(self.ty()?) } else { None };
        let body = self.block()?;
        Ok(FnDecl { comptime, attrs, name, params, ret, body })
    }

    fn kind(&mut self) -> Res<KindDecl> {
        let name = self.ident()?;
        let mut params = vec![];
        if self.eat("[") {
            while !self.is("]") {
                let start = self.span();
                if !self.eat_word("comptime") {
                    return self.err("`comptime` (kind parameters are stage 0)");
                }
                let pname = self.ident()?;
                self.expect(":")?;
                let ty = self.ty()?;
                let range = if self.eat_word("in") {
                    let lo = self.expr_bp(7)?;
                    self.expect("..")?;
                    Some((lo, self.expr_bp(7)?))
                } else {
                    None
                };
                params.push(KindParam { name: pname, ty, range, span: start.to(self.prev_span()) });
                if !self.eat(",") {
                    break;
                }
            }
            self.expect("]")?;
        }
        self.expect("{")?;
        let mut k = KindDecl { name, params, row: vec![], queries: vec![], fns: vec![] };
        while !self.eat("}") {
            let mut attrs = vec![];
            while self.eat("@") {
                let name = self.ident()?;
                let args = if self.is("(") { self.args()?.into_iter().map(|a| a.value).collect() } else { vec![] };
                attrs.push(Attr { name, args });
            }
            if !attrs.is_empty() && !self.is_word("query") {
                return self.err("`query` after an attribute");
            }
            if self.eat_word("row") {
                self.expect("{")?;
                while !self.eat("}") {
                    let fname = self.ident()?;
                    self.expect(":")?;
                    k.row.push(RowField { name: fname, ty: self.ty()? });
                    if !self.eat(",") && !self.eat(";") && !self.is("}") {
                        return self.err("`,` between row fields");
                    }
                }
            } else if self.eat_word("query") {
                let which = match self.peek() {
                    Tok::Ident(n) => n.clone(),
                    Tok::SpaceVar(_) | Tok::Num { .. } | Tok::Punct(_) | Tok::Eof => return self.err("a query name"),
                };
                let which = match which.as_str() {
                    "visible" => QueryKind::Visible,
                    "trace" => QueryKind::Trace,
                    "material" => QueryKind::Material,
                    "surface" => QueryKind::Surface,
                    _ => {
                        return Err(Diag::new(
                            Code::E405,
                            self.span(),
                            "Phase 1 lowers the queries `visible`, `trace`, `surface` and `material` only",
                            "write one of those; `light`, `sky` and `medium` arrive with the light phase",
                        ));
                    }
                };
                // `query trace(self, ...)`: the query's name is its fn's name (draft §6).
                k.queries.push((which, self.fn_rest(false, attrs)?));
            } else if self.eat_word("fn") {
                k.fns.push(self.fn_rest(false, vec![])?);
            } else {
                return self.err("`row`, `query` or `fn` in a kind");
            }
        }
        Ok(k)
    }

    fn expect_word(&mut self, w: &str) -> Res<()> {
        if self.eat_word(w) { Ok(()) } else { self.err(&format!("`{w}`")) }
    }

    fn schedule(&mut self) -> Res<ScheduleDecl> {
        let name = self.ident()?;
        self.expect("{")?;
        let mut passes = vec![];
        while !self.eat("}") {
            self.expect_word("pass")?;
            let label = self.ident()?;
            self.expect(":")?;
            let verb = self.ident()?;
            let args = self.args()?;
            let mut body = vec![];
            if self.eat("{") {
                while !self.eat("}") {
                    let call = self.ident()?;
                    body.push((call, self.args()?));
                    self.expect(";")?;
                }
            }
            self.expect(";")?;
            passes.push(PassDecl { label, verb, args, body });
        }
        Ok(ScheduleDecl { name, passes })
    }

    fn space_ref(&mut self) -> Res<SpaceRef> {
        match self.peek().clone() {
            Tok::Ident(name) => Ok(SpaceRef { name, var: false, span: self.bump().1 }),
            Tok::SpaceVar(name) => Ok(SpaceRef { name, var: true, span: self.bump().1 }),
            Tok::Punct(_) | Tok::Num { .. } | Tok::Eof => self.err("a space"),
        }
    }

    fn ty(&mut self) -> Res<TyExpr> {
        let name = self.ident()?;
        if let Some((_, g)) = GEO.iter().find(|(n, _)| *n == name.name) {
            self.expect("[")?;
            let s = self.space_ref()?;
            self.expect("]")?;
            return Ok(TyExpr::Geo(*g, s));
        }
        match name.name.as_str() {
            "Xform" | "Map" => {
                self.expect("[")?;
                let a = self.space_ref()?;
                self.expect("->")?;
                let b = self.space_ref()?;
                self.expect("]")?;
                if name.name == "Map" {
                    return Err(Diag::new(
                        Code::E405,
                        name.span,
                        "`Map[..]` values come from a `map` statement, never from a declared type",
                        "write `map m = ...;` and use `m.to_parent` / `m.from_parent`",
                    ));
                }
                Ok(TyExpr::Xform(a, b))
            }
            "Frame" => {
                self.expect("[")?;
                let s = self.space_ref()?;
                self.expect("]")?;
                Ok(TyExpr::Frame(s))
            }
            "Field" => {
                self.expect("[")?;
                let s = self.space_ref()?;
                if self.eat(",") {
                    return Err(Diag::new(
                        Code::E211,
                        self.span(),
                        "a field's Lipschitz bound is derived, never authored",
                        "write `Field[S]`; the compiler derives L and reports it with `loft check`",
                    ));
                }
                self.expect("]")?;
                Ok(TyExpr::Field(s))
            }
            _ if self.is("<") => {
                self.bump();
                let mut args = vec![self.ty()?];
                while self.eat(",") {
                    args.push(self.ty()?);
                }
                self.expect(">")?;
                Ok(TyExpr::Generic(name, args))
            }
            _ => Ok(TyExpr::Name(name)),
        }
    }

    fn block(&mut self) -> Res<Block> {
        self.expect("{")?;
        let mut stmts = vec![];
        while !self.eat("}") {
            stmts.push(self.stmt()?);
        }
        Ok(Block { stmts })
    }

    fn stmt(&mut self) -> Res<Stmt> {
        let start = self.span();
        if self.is_word("let") || self.is_word("var") {
            let mutable = self.bump().0 == Tok::Ident("var".into());
            let name = self.ident()?;
            let ty = if self.eat(":") { Some(self.ty()?) } else { None };
            self.expect("=")?;
            let value = self.expr()?;
            self.expect(";")?;
            return Ok(Stmt::Let { mutable, name, ty, value });
        }
        if self.eat_word("map") {
            let name = self.ident()?;
            self.expect("=")?;
            let head = self.expr_bp(1)?;
            let mut stages = vec![];
            while self.eat("|>") {
                let op = self.ident()?;
                stages.push((op, self.args()?.into_iter().map(|a| a.value).collect()));
            }
            self.expect(";")?;
            return Ok(Stmt::Map { name, head, stages });
        }
        if self.eat_word("return") {
            let value = if self.is(";") { None } else { Some(self.expr()?) };
            self.expect(";")?;
            return Ok(Stmt::Return(value, start.to(self.prev_span())));
        }
        if self.eat_word("if") {
            return self.if_rest();
        }
        let comptime = self.eat_word("comptime");
        if self.eat_word("for") {
            let var = self.ident()?;
            self.expect_word("in")?;
            let lo = self.expr_bp(7)?;
            self.expect("..")?;
            let hi = self.expr_bp(7)?;
            let span = start.to(self.prev_span());
            let body = self.block()?;
            return Ok(Stmt::For { comptime, var, lo, hi, body, span });
        }
        if comptime {
            return self.err("`for` after `comptime` in a statement");
        }
        if self.eat_word("emit") {
            let kind = self.ident()?;
            self.expect("{")?;
            let mut fields = vec![];
            while !self.eat("}") {
                let f = self.ident()?;
                self.expect(":")?;
                fields.push((f, self.expr()?));
                if !self.eat(",") && !self.is("}") {
                    return self.err("`,` between fields");
                }
            }
            self.expect(";")?;
            return Ok(Stmt::Emit { kind, fields, span: start.to(self.prev_span()) });
        }
        let e = self.expr()?;
        for (p, op) in [("=", None), ("+=", Some(BinOp::Add)), ("-=", Some(BinOp::Sub)), ("*=", Some(BinOp::Mul))] {
            if self.eat(p) {
                let value = self.expr()?;
                self.expect(";")?;
                return Ok(Stmt::Assign { target: e, op, value });
            }
        }
        self.expect(";")?;
        Ok(Stmt::Expr(e))
    }

    fn if_rest(&mut self) -> Res<Stmt> {
        let cond = self.expr()?;
        let then = self.block()?;
        let otherwise = if self.eat_word("else") {
            if self.eat_word("if") { Some(Block { stmts: vec![self.if_rest()?] }) } else { Some(self.block()?) }
        } else {
            None
        };
        Ok(Stmt::If { cond, then, otherwise })
    }

    fn args(&mut self) -> Res<Vec<Arg>> {
        self.expect("(")?;
        let mut out = vec![];
        while !self.is(")") {
            let name = if matches!(self.peek(), Tok::Ident(_)) && self.peek2() == &Tok::Punct(":") {
                let n = self.ident()?;
                self.bump();
                Some(n)
            } else {
                None
            };
            out.push(Arg { name, value: self.expr()? });
            if !self.eat(",") {
                break;
            }
        }
        self.expect(")")?;
        Ok(out)
    }

    pub fn expr(&mut self) -> Res<Expr> {
        self.expr_bp(0)
    }

    /// Precedence climbing. Levels: 1 `||`, 2 `&&`, 3 `|`, 4 `^`, 5 `&`, 6 comparisons, 7 shifts, 8 `+ -`,
    /// 9 `* / %`. `|>` is only a `map` statement separator. Range bounds parse at level 7 so `..` stops them.
    fn expr_bp(&mut self, min: u8) -> Res<Expr> {
        let mut lhs = self.unary()?;
        loop {
            let (op, bp) = match self.peek() {
                Tok::Punct("||") => (BinOp::Or, 1),
                Tok::Punct("&&") => (BinOp::And, 2),
                Tok::Punct("|") => (BinOp::BitOr, 3),
                Tok::Punct("^") => (BinOp::BitXor, 4),
                Tok::Punct("&") => (BinOp::BitAnd, 5),
                Tok::Punct("==") => (BinOp::Eq, 6),
                Tok::Punct("!=") => (BinOp::Ne, 6),
                Tok::Punct("<") => (BinOp::Lt, 6),
                Tok::Punct("<=") => (BinOp::Le, 6),
                Tok::Punct(">") => (BinOp::Gt, 6),
                Tok::Punct(">=") => (BinOp::Ge, 6),
                Tok::Punct("<<") => (BinOp::Shl, 7),
                Tok::Punct(">>") => (BinOp::Shr, 7),
                Tok::Punct("+") => (BinOp::Add, 8),
                Tok::Punct("-") => (BinOp::Sub, 8),
                Tok::Punct("*") => (BinOp::Mul, 9),
                Tok::Punct("/") => (BinOp::Div, 9),
                Tok::Punct("%") => (BinOp::Rem, 9),
                Tok::Ident(_) | Tok::SpaceVar(_) | Tok::Num { .. } | Tok::Punct(_) | Tok::Eof => break,
            };
            if bp <= min {
                break;
            }
            self.bump();
            let rhs = self.expr_bp(bp)?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr { id: self.id(), span, kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)) };
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Res<Expr> {
        let start = self.span();
        let op = if self.eat("-") {
            Some(UnOp::Neg)
        } else if self.eat("!") {
            Some(UnOp::Not)
        } else {
            None
        };
        if let Some(op) = op {
            let e = self.unary()?;
            let span = start.to(e.span);
            return Ok(Expr { id: self.id(), span, kind: ExprKind::Unary(op, Box::new(e)) });
        }
        let mut e = self.primary()?;
        loop {
            if self.eat(".") {
                let m = self.ident()?;
                let span = e.span.to(m.span);
                e = Expr { id: self.id(), span, kind: ExprKind::Member(Box::new(e), m) };
            } else if self.eat("[") {
                let i = self.expr()?;
                let end = self.expect("]")?;
                let span = e.span.to(end);
                e = Expr { id: self.id(), span, kind: ExprKind::Index(Box::new(e), Box::new(i)) };
            } else {
                return Ok(e);
            }
        }
    }

    fn primary(&mut self) -> Res<Expr> {
        let start = self.span();
        match self.peek().clone() {
            Tok::Num { value, suffix, text, float } => {
                self.bump();
                Ok(Expr { id: self.id(), span: start, kind: ExprKind::Num { value, suffix, text, float } })
            }
            Tok::Punct("(") => {
                self.bump();
                let e = self.expr()?;
                self.expect(")")?;
                Ok(e)
            }
            Tok::Ident(name) => {
                let is_ctor = (GEO.iter().any(|(n, _)| *n == name) || name == "Xform" || name == "Frame") && self.peek2() == &Tok::Punct("[")
                    || (name.starts_with("vec") || name == "array") && self.peek2() == &Tok::Punct("<");
                if is_ctor {
                    let ty = self.ty()?;
                    let args = self.args()?;
                    let span = start.to(self.prev_span());
                    return Ok(Expr { id: self.id(), span, kind: ExprKind::Ctor { ty, args } });
                }
                let callee = self.ident()?;
                // `diff(f, x)(args)`: the derivative of fn `f` in its parameter `x`, a fn the AD pass generates.
                if name == "diff" && self.is("(") {
                    self.expect("(")?;
                    let f = self.ident()?;
                    self.expect(",")?;
                    let x = self.ident()?;
                    self.expect(")")?;
                    let args = self.args()?;
                    let span = start.to(self.prev_span());
                    let callee = Ident { name: format!("{}__d_{}", f.name, x.name), span: f.span };
                    self.diffs.push((f, x));
                    return Ok(Expr { id: self.id(), span, kind: ExprKind::Call { callee, args } });
                }
                match name.as_str() {
                    "true" | "false" => Ok(Expr { id: self.id(), span: start, kind: ExprKind::Bool(name == "true") }),
                    _ if self.is("(") => {
                        let args = self.args()?;
                        let span = start.to(self.prev_span());
                        Ok(Expr { id: self.id(), span, kind: ExprKind::Call { callee, args } })
                    }
                    _ => Ok(Expr { id: self.id(), span: start, kind: ExprKind::Name(name) }),
                }
            }
            Tok::Punct(_) | Tok::SpaceVar(_) | Tok::Eof => self.err("an expression"),
        }
    }
}
