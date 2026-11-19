//! Recursive descent over the lossless CST. Newlines are virtual separators;
//! all expression and diagnostic spans still address the original UTF-8.
use crate::cst::{Cst, Kind, Trivia};
use loft_ir::ast::*;
use loft_ir::{Code, Diag, Res, Span};
use std::collections::HashSet;

#[derive(Clone, Copy)]
struct Lex<'a> {
    text: &'a str,
    span: Span,
    kind: Kind,
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
        || matches!(next, "|>" | "else")
}

pub fn parse(file: u16, source: &str, next: &mut u32) -> Res<Module> {
    let tree = Cst::parse(file, source)?;
    let mut tokens = vec![];
    let mut depth = 0usize;
    let mut prev = "";
    for t in tree.tokens() {
        let word = t.text(source);
        if (!continued(prev, word, depth)
            || (prev == "*"
                && tokens
                    .iter()
                    .rev()
                    .nth(1)
                    .is_some_and(|t: &Lex| t.text == "::")))
            && let Some((_, s)) = t.leading.iter().find(|(k, _)| *k == Trivia::Newline)
        {
            tokens.push(Lex {
                text: ";",
                span: *s,
                kind: Kind::Newline,
            });
        }
        tokens.push(Lex {
            text: word,
            span: t.span,
            kind: t.kind,
        });
        match word {
            "(" | "[" => depth += 1,
            ")" | "]" => depth -= 1,
            _ => (),
        }
        prev = word;
    }
    let mut p = Parser {
        tokens,
        at: 0,
        next,
        module: Module::default(),
        generics: HashSet::new(),
        fields: HashSet::new(),
        locals: HashSet::new(),
        maps: HashSet::new(),
        derivatives: vec![],
    };
    p.seps();
    while !p.is("") {
        let item = p.item()?;
        p.module.items.push(item);
        p.end()?;
    }
    p.module.diffs = p.derivatives;
    Ok(p.module)
}

struct Parser<'a, 'b> {
    tokens: Vec<Lex<'a>>,
    at: usize,
    next: &'b mut u32,
    module: Module,
    generics: HashSet<String>,
    fields: HashSet<String>,
    locals: HashSet<String>,
    maps: HashSet<String>,
    derivatives: Vec<(Ident, Ident)>,
}
fn named(name: &str, span: Span) -> Ident {
    Ident {
        name: name.into(),
        span,
    }
}
fn ty(name: &str, span: Span) -> TyExpr {
    TyExpr::Name(named(name, span))
}
impl Parser<'_, '_> {
    fn cur(&self) -> Lex<'_> {
        self.tokens[self.at]
    }
    fn is(&self, s: &str) -> bool {
        self.cur().text == s
    }
    fn pop(&mut self) -> Lex<'_> {
        let i = self.at;
        if self.tokens[i].kind != Kind::End {
            self.at += 1;
        }
        self.tokens[i]
    }
    fn eat(&mut self, s: &str) -> bool {
        if self.is(s) {
            self.pop();
            true
        } else {
            false
        }
    }
    fn err<T>(&self, msg: &str) -> Res<T> {
        Err(Diag::new(
            Code::E001,
            self.cur().span,
            format!("expected {msg}, found `{}`", self.cur().text),
            format!("write {msg} here"),
        ))
    }
    fn expect(&mut self, s: &str) -> Res<()> {
        if self.eat(s) { Ok(()) } else { self.err(s) }
    }
    fn seps(&mut self) {
        while self.eat(";") {}
    }
    fn end(&mut self) -> Res<()> {
        if self.is("}") || self.is("") {
            return Ok(());
        }
        self.expect(";")?;
        self.seps();
        Ok(())
    }
    fn ident(&mut self) -> Res<Ident> {
        if self.cur().kind != Kind::Word {
            return self.err("an identifier");
        }
        let t = self.pop();
        Ok(named(t.text, t.span))
    }
    fn node(&mut self, span: Span, kind: ExprKind) -> Expr {
        let id = *self.next;
        *self.next += 1;
        Expr { id, span, kind }
    }
    fn num(&mut self, value: f64, float: bool, span: Span) -> Expr {
        self.node(
            span,
            ExprKind::Num {
                value,
                suffix: NumSuffix::None,
                text: value.to_string(),
                float,
            },
        )
    }
    fn space(&mut self) -> Res<SpaceRef> {
        let n = self.ident()?;
        Ok(SpaceRef {
            var: self.generics.contains(&n.name),
            name: n.name,
            span: n.span,
        })
    }
    fn ty(&mut self) -> Res<TyExpr> {
        if self.eat("[") {
            let t = self.ident()?;
            self.expect("]")?;
            return Ok(TyExpr::Table(t));
        }
        let n = self.ident()?;
        if self.eat("@") {
            let a = self.space()?;
            return Ok(match n.name.as_str() {
                "xf" => {
                    self.expect(">")?;
                    TyExpr::Xform(a, self.space()?)
                }
                "frame" => TyExpr::Frame(a),
                "p3" | "v3" | "d3" | "n3" => TyExpr::Geo(
                    match n.name.as_str() {
                        "p3" => Geo::Pt,
                        "v3" => Geo::Vec,
                        "d3" => Geo::Dir,
                        _ => Geo::Norm,
                    },
                    a,
                ),
                _ => return self.err("a space-qualified geometry type"),
            });
        }
        let name = match n.name.as_str() {
            "f" => "f32",
            "u" => "u32",
            "i" => "i32",
            "v2" => "vec2f",
            "rgb" | "v3" => "vec3f",
            "v4" => "vec4f",
            "m" | "mm" | "deg" | "rad" | "s" | "kg" | "N" | "Hz" | "J" => {
                while self.is("/") || self.is("*") {
                    self.pop();
                    let u = self.ident()?;
                    if !unit(&u.name) {
                        return self.err("a unit");
                    }
                    if self.eat("^") {
                        self.eat("-");
                        self.integer()?;
                    }
                }
                "f32"
            }
            _ => &n.name,
        };
        Ok(ty(name, n.span))
    }
    fn integer(&mut self) -> Res<u32> {
        let t = self.cur();
        let n = t.text.parse().map_err(|_| {
            Diag::new(
                Code::E001,
                t.span,
                "expected a nonnegative integer",
                "write an integer literal",
            )
        })?;
        self.pop();
        Ok(n)
    }
    fn item(&mut self) -> Res<Item> {
        let public = self.eat("pub");
        let build = self.eat("build");
        let start = self.cur().span;
        if build && self.is("{") {
            let name = named(&format!("loft_build_{}_{}", start.file, start.lo), start);
            let body = self.block(false)?;
            return Ok(Item::Build(FnDecl {
                comptime: true,
                attrs: vec![],
                name,
                params: vec![],
                ret: None,
                body,
                raw: None,
            }));
        }
        if self.eat("fn") {
            return Ok(Item::Fn(self.function(build, public, false)?));
        }
        if build {
            return self.err("a build block or build fn");
        }
        if self.eat("lobe") {
            return Ok(Item::Fn(self.function(false, public, true)?));
        }
        if self.eat("space") {
            let mut ns = vec![self.ident()?];
            while self.cur().kind == Kind::Word {
                ns.push(self.ident()?)
            }
            return Ok(Item::Space(ns));
        }
        let parameter = self.eat("param");
        if parameter || self.eat("const") {
            let name = self.ident()?;
            let t = if self.eat(":") {
                Some(self.ty()?)
            } else {
                None
            };
            self.expect("=")?;
            let value = self.expr(0)?;
            return Ok(Item::Const(ConstDecl {
                range: if parameter { self.range()? } else { None },
                comptime: parameter,
                name,
                ty: t,
                value,
            }));
        }
        if self.eat("kind") {
            return self.kind().map(Item::Kind);
        }
        if self.eat("schedule") {
            let name = self.ident()?;
            self.expect("{")?;
            self.seps();
            let mut passes = vec![];
            while !self.eat("}") {
                let verb = self.ident()?;
                let args = self.args()?;
                let mut body = vec![];
                if self.eat("{") {
                    self.seps();
                    while !self.eat("}") {
                        let f = self.ident()?;
                        body.push((f, self.args()?));
                        self.end()?
                    }
                }
                passes.push(PassDecl {
                    label: verb.clone(),
                    verb,
                    args,
                    body,
                });
                self.end()?
            }
            return Ok(Item::Schedule(ScheduleDecl { name, passes }));
        }
        if self.eat("camera") {
            let name = self.ident()?;
            self.expect("{")?;
            let fields = self.fields("}")?;
            return Ok(Item::Camera(CameraDecl { name, fields }));
        }
        if self.eat("scan") {
            let name = self.ident()?;
            let var = self.ident()?;
            self.expect("in")?;
            let lo = self.expr(0)?;
            self.expect("..")?;
            let hi = self.expr(0)?;
            self.expect("by")?;
            let steps = self.primary()?;
            self.expect("on")?;
            self.expect("bake_lut")?;
            self.expect("{")?;
            self.seps();
            self.expect("init")?;
            self.expect("=")?;
            let init = self.expr(0)?;
            self.end()?;
            self.expect("step")?;
            let prev = self.ident()?;
            self.expect("=")?;
            let step = self.expr(0)?;
            self.end()?;
            self.expect("}")?;
            return Ok(Item::Scan(ScanDecl {
                name,
                at: var.clone(),
                var_ty: ty("f32", var.span),
                var,
                lo,
                hi,
                steps,
                init,
                prev,
                step,
                elem: None,
            }));
        }
        if self.eat("law") {
            let name = self.ident()?;
            let params = self.params()?;
            self.seps();
            self.expect("writes")?;
            let owner = self.ident()?;
            if params.first().map(|p| &p.name.name) != Some(&owner.name) {
                return Err(Diag::new(
                    Code::E501,
                    owner.span,
                    "writes targets a different row",
                    "name the first law parameter",
                ));
            }
            self.expect(".")?;
            self.expect("{")?;
            let mut writes = vec![];
            while !self.eat("}") {
                writes.push(self.ident()?);
                if !self.eat(",") {
                    self.expect("}")?;
                    break;
                }
            }
            self.seps();
            let body = self.block(false)?;
            return Ok(Item::Law(LawDecl {
                f: FnDecl {
                    comptime: false,
                    attrs: vec![],
                    name,
                    params,
                    ret: None,
                    body,
                    raw: None,
                },
                writes,
            }));
        }
        if self.eat("check") {
            let name = self.ident()?;
            let params = self.params()?;
            self.expect("->")?;
            let ret = Some(self.ty()?);
            let body = self.body(true)?;
            return Ok(Item::Check(FnDecl {
                comptime: false,
                attrs: vec![],
                name,
                params,
                ret,
                body,
                raw: None,
            }));
        }
        if self.eat("use") {
            let mut path = vec![self.ident()?];
            while self.eat("::") {
                if self.eat("*") {
                    break;
                }
                path.push(self.ident()?)
            }
            return Ok(Item::Use(path));
        }
        Err(Diag::new(
            if matches!(
                self.cur().text,
                "body" | "state" | "solver" | "plan" | "target" | "world" | "retarget"
            ) {
                Code::E405
            } else {
                Code::E404
            },
            start,
            format!("unsupported module item `{}`", self.cur().text),
            "declare a v1 item or put row generation inside build { ... }",
        ))
    }
    fn params(&mut self) -> Res<Vec<Param>> {
        self.expect("(")?;
        let mut out = vec![];
        while !self.eat(")") {
            let name = self.ident()?;
            let ty = if self.eat(":") {
                Some(self.ty()?)
            } else {
                None
            };
            out.push(Param { name, ty });
            if !self.eat(",") {
                self.expect(")")?;
                break;
            }
        }
        Ok(out)
    }
    fn function(&mut self, build: bool, public: bool, lobe: bool) -> Res<FnDecl> {
        let n = self.ident()?;
        self.generics.clear();
        if self.eat("[") {
            loop {
                let s = self.ident()?;
                self.generics.insert(s.name);
                if !self.eat(",") {
                    break;
                }
            }
            self.expect("]")?
        }
        let params = self.params()?;
        let ret = if self.eat("->") {
            Some(self.ty()?)
        } else if lobe {
            Some(ty("vec4f", n.span))
        } else {
            None
        };
        if public && !lobe && (params.iter().any(|p| p.ty.is_none()) || ret.is_none() && !build) {
            return Err(Diag::new(
                Code::E003,
                n.span,
                "public function needs a signature",
                "annotate its parameters and result",
            ));
        }
        let attrs = if lobe {
            vec![Attr {
                name: named("lobe", n.span),
                args: vec![],
            }]
        } else {
            vec![]
        };
        self.locals = params.iter().map(|p| p.name.name.clone()).collect();
        let body = self.body(!build || ret.is_some())?;
        self.generics.clear();
        self.locals.clear();
        Ok(FnDecl {
            comptime: build,
            attrs,
            name: n,
            params,
            ret,
            body,
            raw: None,
        })
    }
    fn range(&mut self) -> Res<Option<RowRange>> {
        Ok(if self.eat("in") {
            let lo = self.expr(0)?;
            self.expect("..")?;
            Some(RowRange {
                lo,
                hi: Some(self.expr(0)?),
                strict: false,
            })
        } else if self.is(">") || self.is(">=") {
            let strict = self.eat(">");
            if !strict {
                self.expect(">=")?;
            }
            Some(RowRange {
                lo: self.expr(0)?,
                hi: None,
                strict,
            })
        } else {
            None
        })
    }
    fn kind(&mut self) -> Res<KindDecl> {
        let name = self.ident()?;
        let mut params = vec![];
        if self.eat("[") {
            loop {
                let n = self.ident()?;
                self.expect(":")?;
                let t = self.ty()?;
                params.push(KindParam {
                    name: n.clone(),
                    ty: t,
                    range: if self.eat("in") {
                        let lo = self.expr(0)?;
                        self.expect("..")?;
                        Some((lo, self.expr(0)?))
                    } else {
                        None
                    },
                    span: n.span,
                });
                if !self.eat(",") {
                    break;
                }
            }
            self.expect("]")?
        }
        self.expect("@")?;
        let space = self.space()?;
        self.expect("(")?;
        let mut row = vec![];
        while !self.eat(")") {
            let name = self.ident()?;
            self.expect(":")?;
            let t = self.ty()?;
            let range = self.range()?;
            let default = if self.eat("=") {
                Some(self.expr(0)?)
            } else {
                None
            };
            row.push(RowField {
                name,
                ty: t,
                range,
                default,
            });
            if !self.eat(",") {
                self.expect(")")?;
                break;
            }
        }
        self.fields = row.iter().map(|f| f.name.name.clone()).collect();
        self.expect("{")?;
        self.seps();
        let mut fns = vec![];
        let mut queries = vec![];
        let mut maps = vec![];
        while !self.eat("}") {
            if self.eat("map") {
                let n = self.ident()?;
                self.expect("(")?;
                let p = self.ident()?;
                self.expect(")")?;
                self.expect("=")?;
                let pos = self.at;
                self.map(n.clone())?;
                maps.push((n, p, pos));
            } else if self.eat("fn") {
                fns.push(self.function(false, false, false)?);
            } else {
                let n = self.ident()?;
                let q = match n.name.as_str() {
                    "trace" => QueryKind::Trace,
                    "material" => QueryKind::Material,
                    "visible" => QueryKind::Visible,
                    "surface" => QueryKind::Surface,
                    "lipschitz" => {
                        return Err(Diag::new(
                            Code::E210,
                            n.span,
                            "authored Lipschitz bound",
                            "remove the bound and refine the row columns",
                        ));
                    }
                    _ => {
                        return Err(Diag::new(
                            Code::E405,
                            n.span,
                            "unknown or unsupported kind query",
                            "write trace, material, visible or surface",
                        ));
                    }
                };
                let mut ps = self.params()?;
                let scalar = ty("f32", n.span);
                let geo = |g| TyExpr::Geo(g, space.clone());
                let (sig, ret) = match q {
                    QueryKind::Trace => (vec![geo(Geo::Pt)], TyExpr::Field(space.clone())),
                    QueryKind::Visible => (vec![geo(Geo::Pt), geo(Geo::Dir)], scalar.clone()),
                    QueryKind::Surface => (vec![scalar.clone(), scalar], geo(Geo::Pt)),
                    QueryKind::Material => (
                        vec![geo(Geo::Pt), geo(Geo::Norm), ty("vec2f", n.span)],
                        ty("Bsdf", n.span),
                    ),
                };
                for (p, t) in ps.iter_mut().zip(sig) {
                    p.ty.get_or_insert(t);
                }
                self.locals = ps.iter().map(|p| p.name.name.clone()).collect();
                let mut attrs = vec![];
                if q == QueryKind::Surface {
                    self.expect("grid")?;
                    let t = self.cur();
                    let span = t.span;
                    let (x, y) = t.text.split_once('x').ok_or_else(|| {
                        Diag::new(Code::E001, span, "invalid grid", "write nu x nv as 96x32")
                    })?;
                    let (x, y) = (x.parse::<u32>(), y.parse::<u32>());
                    self.pop();
                    let (Ok(x), Ok(y)) = (x, y) else {
                        return self.err("a grid dimension");
                    };
                    attrs.push(Attr {
                        name: named("grid", span),
                        args: vec![
                            self.num(x.into(), false, span),
                            self.num(y.into(), false, span),
                        ],
                    });
                }
                let query_pos = self.at;
                let mut pre = vec![];
                self.maps.clear();
                if matches!(q, QueryKind::Trace | QueryKind::Material) {
                    for (name, formal, pos) in &maps {
                        self.at = *pos;
                        let mut m = self.map(name.clone())?;
                        if let Stmt::Map { head, .. } = &mut m
                            && matches!(&head.kind,ExprKind::Name(x) if x==&formal.name)
                            && let Some(point) = ps.first()
                        {
                            head.kind = ExprKind::Name(point.name.name.clone());
                        }
                        self.maps.insert(name.name.clone());
                        pre.push(m);
                    }
                }
                self.at = query_pos;
                let mut b = self.body(true)?;
                pre.append(&mut b.stmts);
                b.stmts = pre;
                ps.insert(
                    0,
                    Param {
                        name: named("self", n.span),
                        ty: None,
                    },
                );
                queries.push((
                    q,
                    FnDecl {
                        comptime: false,
                        attrs,
                        name: n,
                        params: ps,
                        ret: Some(ret),
                        body: b,
                        raw: None,
                    },
                ));
                self.maps.clear();
                self.locals.clear();
            }
            self.end()?;
        }
        self.fields.clear();
        Ok(KindDecl {
            name,
            params,
            row,
            queries,
            fns,
        })
    }
    fn body(&mut self, value: bool) -> Res<Block> {
        let eq = self.eat("=");
        if self.is("{") {
            return self.block(value);
        }
        if !eq {
            return self.err("= expression or a block");
        }
        let e = self.expr(0)?;
        let sp = e.span;
        Ok(Block {
            stmts: vec![Stmt::Return(Some(e), sp)],
        })
    }
    fn block(&mut self, value: bool) -> Res<Block> {
        self.expect("{")?;
        self.seps();
        let locals = self.locals.clone();
        let maps = self.maps.clone();
        let mut stmts = vec![];
        while !self.eat("}") {
            let s = self.stmt()?;
            self.end()?;
            stmts.push(s)
        }
        fn tail(stmts: &mut [Stmt]) {
            match stmts.last_mut() {
                Some(s @ Stmt::Expr(_)) => {
                    let Stmt::Expr(e) = std::mem::replace(s, Stmt::Return(None, Span::NONE)) else {
                        unreachable!()
                    };
                    *s = Stmt::Return(Some(e.clone()), e.span);
                }
                Some(Stmt::If {
                    then,
                    otherwise: Some(no),
                    ..
                }) => {
                    tail(&mut then.stmts);
                    tail(&mut no.stmts);
                }
                _ => {}
            }
        }
        if value {
            tail(&mut stmts);
        }
        self.locals = locals;
        self.maps = maps;
        Ok(Block { stmts })
    }
    fn fields(&mut self, end: &str) -> Res<Vec<(Ident, Expr)>> {
        self.seps();
        let mut fs = vec![];
        while !self.eat(end) {
            let n = self.ident()?;
            self.expect(":")?;
            fs.push((n, self.expr(0)?));
            if !self.eat(",") && !self.is(";") && !self.is(end) {
                return self.err("a field separator");
            }
            self.seps()
        }
        Ok(fs)
    }
    fn emit(&mut self, bind: Option<Ident>, span: Span) -> Res<Stmt> {
        let kind = self.ident()?;
        self.expect("(")?;
        let fields = self.fields(")")?;
        Ok(Stmt::Emit {
            kind,
            fields,
            span,
            bind,
        })
    }
    fn stmt(&mut self) -> Res<Stmt> {
        let sp = self.cur().span;
        if self.is("let") || self.is("var") {
            let mutable = self.eat("var");
            if !mutable {
                self.expect("let")?;
            }
            let name = self.ident()?;
            let t = if self.eat(":") {
                Some(self.ty()?)
            } else {
                None
            };
            self.expect("=")?;
            if self.eat("emit") {
                return self.emit(Some(name), sp);
            }
            let value = self.expr(0)?;
            self.locals.insert(name.name.clone());
            return Ok(Stmt::Let {
                mutable,
                name,
                ty: t,
                value,
            });
        }
        if self.eat("map") {
            let n = self.ident()?;
            self.expect("=")?;
            let m = self.map(n.clone())?;
            self.maps.insert(n.name);
            return Ok(m);
        }
        if self.eat("return") {
            return Ok(Stmt::Return(Some(self.expr(0)?), sp));
        }
        if self.eat("emit") {
            return self.emit(None, sp);
        }
        let build = self.eat("build");
        if build || self.is("for") {
            self.expect("for")?;
            let var = self.ident()?;
            self.expect("in")?;
            if build && self.eat("[") {
                let values = self.list("]")?.into_iter().map(|arg| arg.value).collect();
                let body = self.block(false)?;
                return Ok(Stmt::Each {
                    var,
                    values,
                    body,
                    span: sp,
                });
            }
            let lo = self.expr(0)?;
            self.expect("..")?;
            let hi = self.expr(0)?;
            let body = self.block(false)?;
            return Ok(Stmt::For {
                comptime: build,
                var,
                lo,
                hi,
                body,
                span: sp,
            });
        }
        if self.eat("if") {
            let cond = self.expr(0)?;
            let then = self.block(false)?;
            let otherwise = if self.eat("else") {
                Some(self.block(false)?)
            } else {
                None
            };
            return Ok(Stmt::If {
                cond,
                then,
                otherwise,
            });
        }
        let e = self.expr(0)?;
        if self.eat("=") {
            Ok(Stmt::Assign {
                target: e,
                op: None,
                value: self.expr(0)?,
            })
        } else {
            Ok(Stmt::Expr(e))
        }
    }
    fn map(&mut self, name: Ident) -> Res<Stmt> {
        let head = self.expr(0)?;
        let mut stages = vec![];
        while self.eat("|>") {
            let mut op = self.ident()?;
            if op.name == "move" {
                op.name = "translate".into()
            }
            let args = if self.is("(") {
                self.args()?.into_iter().map(|a| a.value).collect()
            } else {
                vec![]
            };
            stages.push((op, args))
        }
        Ok(Stmt::Map { name, head, stages })
    }
    fn args(&mut self) -> Res<Vec<Arg>> {
        self.expect("(")?;
        self.list(")")
    }
    fn list(&mut self, end: &str) -> Res<Vec<Arg>> {
        let mut a = vec![];
        while !self.eat(end) {
            let name = if self.tokens.get(self.at + 1).is_some_and(|t| t.text == ":") {
                let n = self.ident()?;
                self.expect(":")?;
                Some(n)
            } else {
                None
            };
            a.push(Arg {
                name,
                value: self.expr(0)?,
            });
            if !self.eat(",") {
                self.expect(end)?;
                break;
            }
        }
        Ok(a)
    }
    fn expr(&mut self, min: u8) -> Res<Expr> {
        let sp = self.cur().span;
        let mut a = if self.is("-") || self.is("!") {
            let op = if self.eat("-") {
                UnOp::Neg
            } else {
                self.pop();
                UnOp::Not
            };
            let b = self.expr(10)?;
            self.node(sp.to(b.span), ExprKind::Unary(op, Box::new(b)))
        } else {
            self.primary()?
        };
        loop {
            if self.eat(".") {
                let mut m = self.ident()?;
                let s = a.span.to(m.span);
                if let ExprKind::Name(n) = &a.kind
                    && self.maps.contains(n)
                {
                    match m.name.as_str() {
                        "p" => continue,
                        "pt" => {
                            let ty = TyExpr::Geo(
                                Geo::Pt,
                                SpaceRef {
                                    name: n.clone(),
                                    var: true,
                                    span: m.span,
                                },
                            );
                            let args = self.args()?;
                            a = self.node(s, ExprKind::Ctor { ty, args });
                            continue;
                        }
                        "out" | "in" => {
                            m.name = if m.name == "out" {
                                "to_parent"
                            } else {
                                "from_parent"
                            }
                            .into();
                            let op = self.node(s, ExprKind::Member(Box::new(a), m));
                            self.expect("(")?;
                            let x = self.expr(0)?;
                            self.expect(")")?;
                            a = self.node(
                                s.to(x.span),
                                ExprKind::Binary(BinOp::Mul, Box::new(op), Box::new(x)),
                            );
                            continue;
                        }
                        _ => (),
                    }
                }
                a = self.node(s, ExprKind::Member(Box::new(a), m));
                continue;
            }
            if self.eat("[") {
                let i = self.expr(0)?;
                self.expect("]")?;
                a = self.node(sp.to(i.span), ExprKind::Index(Box::new(a), Box::new(i)));
                continue;
            }
            let Some((bp, op)) = operator(self.cur().text) else {
                break;
            };
            if bp <= min {
                break;
            }
            self.pop();
            let b = self.expr(bp)?;
            a = self.node(
                sp.to(b.span),
                ExprKind::Binary(op, Box::new(a), Box::new(b)),
            );
        }
        Ok(a)
    }
    fn primary(&mut self) -> Res<Expr> {
        let span = self.cur().span;
        if self.eat("if") {
            let cond = self.expr(0)?;
            let arm = |p: &mut Self| -> Res<Expr> {
                p.expect("{")?;
                p.seps();
                let v = p.expr(0)?;
                p.seps();
                p.expect("}")?;
                Ok(v)
            };
            let yes = arm(self)?;
            self.expect("else")?;
            let no = arm(self)?;
            let args = [no, yes, cond]
                .into_iter()
                .map(|value| Arg { name: None, value })
                .collect();
            return Ok(self.node(
                span,
                ExprKind::Call {
                    callee: named("loft_conditional", span),
                    args,
                },
            ));
        }
        if self.eat("(") {
            let e = self.expr(0)?;
            self.expect(")")?;
            return Ok(e);
        }
        if self.eat("[") {
            let args = self.list("]")?;
            let ty = if self.eat("@") {
                TyExpr::Geo(Geo::Pt, self.space()?)
            } else {
                match args.len() {
                    2 => ty("vec2f", span),
                    3 => ty("vec3f", span),
                    4 => ty("vec4f", span),
                    _ => return self.err("two to four vector components"),
                }
            };
            return Ok(self.node(span, ExprKind::Ctor { ty, args }));
        }
        if self.cur().kind == Kind::Number {
            let s = self.pop().text.to_owned();
            return self.number(&s, span);
        }
        let mut n = self.ident()?;
        if n.name == "true" || n.name == "false" {
            return Ok(self.node(span, ExprKind::Bool(n.name == "true")));
        }
        if self.eat("'") || n.name == "diff" && self.is("(") {
            let (f, x) = if n.name == "diff" {
                self.expect("(")?;
                let f = self.ident()?;
                self.expect(",")?;
                let x = self.ident()?;
                self.expect(")")?;
                (f, x)
            } else {
                (n.clone(), named("", span))
            };
            n.name = format!("{}__d_{}", f.name, x.name);
            self.derivatives.push((f, x));
        }
        if self.is("(") {
            let mut args = self.args()?;
            if matches!(n.name.as_str(), "rgb" | "v2" | "v3" | "v4") {
                let target = match n.name.as_str() {
                    "v2" => "vec2f",
                    "v4" => "vec4f",
                    _ => "vec3f",
                };
                return Ok(self.node(
                    span,
                    ExprKind::Ctor {
                        ty: ty(target, span),
                        args,
                    },
                ));
            }
            if let Some(op) = match n.name.as_str() {
                "xor" => Some(BinOp::BitXor),
                "shr" => Some(BinOp::Shr),
                "bit_and" => Some(BinOp::BitAnd),
                _ => None,
            } {
                if args.len() != 2 {
                    return self.err("two integer operands");
                }
                let rhs = args.pop().unwrap().value;
                let lhs = args.pop().unwrap().value;
                return Ok(self.node(span, ExprKind::Binary(op, Box::new(lhs), Box::new(rhs))));
            }
            return Ok(self.node(span, ExprKind::Call { callee: n, args }));
        }
        if self.fields.contains(&n.name) && !self.locals.contains(&n.name) {
            let base = self.node(span, ExprKind::Name("self".into()));
            return Ok(self.node(span, ExprKind::Member(Box::new(base), n)));
        }
        Ok(self.node(span, ExprKind::Name(n.name)))
    }
    fn number(&mut self, s: &str, sp: Span) -> Res<Expr> {
        let bad = || {
            Diag::new(
                Code::E001,
                sp,
                format!("invalid number or unit `{s}`"),
                "use a decimal number and a declared unit symbol",
            )
        };
        if let Some(hex) = s.strip_prefix("0x") {
            return Ok(self.num(
                u32::from_str_radix(hex, 16).map_err(|_| bad())?.into(),
                false,
                sp,
            ));
        }
        let mut end = 0;
        for (i, ch) in s.char_indices() {
            if ch.is_ascii_digit() || ch == '.' || matches!(ch, 'e' | 'E' | '+' | '-') {
                end = i + 1
            } else {
                break;
            }
        }
        let num = &s[..end];
        let mut value = num.parse::<f64>().map_err(|_| bad())?;
        let units = &s[end..];
        let suffix = if units.is_empty() {
            NumSuffix::None
        } else {
            let mut op = 1;
            let mut at = 0;
            while at < units.len() {
                let rest = &units[at..];
                let n = rest.bytes().take_while(u8::is_ascii_alphabetic).count();
                let sym = &rest[..n];
                if !unit(sym) {
                    return Err(bad());
                }
                at += n;
                let mut power = 1;
                if units[at..].starts_with('^') {
                    at += 1;
                    let start = at;
                    if units[at..].starts_with('-') {
                        at += 1
                    }
                    at += units[at..].bytes().take_while(u8::is_ascii_digit).count();
                    power = units[start..at].parse::<i32>().map_err(|_| bad())?
                }
                let factor: f64 = match sym {
                    "mm" => 0.001,
                    "deg" => std::f64::consts::PI / 180.,
                    _ => 1.,
                };
                value *= factor.powi(op * power);
                if at < units.len() {
                    op = match units.as_bytes()[at] {
                        b'/' => -1,
                        b'*' => 1,
                        _ => return Err(bad()),
                    };
                    at += 1;
                }
            }
            NumSuffix::Unit
        };
        Ok(self.node(
            sp,
            ExprKind::Num {
                value,
                suffix,
                text: s.to_string(),
                float: num.contains(['.', 'e', 'E']) || !units.is_empty(),
            },
        ))
    }
}
fn unit(s: &str) -> bool {
    matches!(
        s,
        "m" | "mm" | "rad" | "deg" | "kg" | "s" | "N" | "J" | "Hz"
    )
}
fn operator(s: &str) -> Option<(u8, BinOp)> {
    use BinOp::*;
    Some(match s {
        "||" => (1, Or),
        "&&" => (2, And),
        "==" => (3, Eq),
        "!=" => (3, Ne),
        "<" => (3, Lt),
        "<=" => (3, Le),
        ">" => (3, Gt),
        ">=" => (3, Ge),
        "|" => (4, BitOr),
        "^" => (5, BitXor),
        "&" => (6, BitAnd),
        "<<" => (7, Shl),
        ">>" => (7, Shr),
        "+" => (8, Add),
        "-" => (8, Sub),
        "*" => (9, Mul),
        "/" => (9, Div),
        "%" => (9, Rem),
        _ => return None,
    })
}
