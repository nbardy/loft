//! Recursive descent over flat tokens. Newlines are virtual separators;
//! all expression and diagnostic spans still address the original UTF-8.
use crate::lex::{Kind, Lex, lex, unit};
use crate::ast::*;
use loft_ir::{Code, Diag, Res, Span};
use std::collections::HashSet;

pub fn parse(file: u16, source: &str, next: &mut u32) -> Res<Module> {
    let mut p = Parser {
        tokens: lex(file, source)?,
        at: 0,
        next,
        module: Module { file: Some(file), ..Module::default() },
        generics: HashSet::new(),
        fields: HashSet::new(),
        locals: HashSet::new(),
        maps: HashSet::new(),
        geometry: false,
        in_opt_out: false,
        proof: GeometryProof::Required,
        derivatives: vec![],
        kind_helpers: Default::default(),
    };
    p.seps();
    while !p.is("") {
        let public = p.eat("pub");
        let item = p.item(public)?;
        let names = item.declarations();
        if let Some(body) = names.first().and_then(|n| p.module.bodies.get_mut(&n.span)) {
            body.hi = p.tokens[p.at - 1].span.hi;
        }
        if public {
            if names.is_empty() {
                return p.err("a named declaration after pub");
            }
            p.module.public.extend(names.iter().map(|n| n.span));
        }
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
    geometry: bool,
    in_opt_out: bool,
    proof: GeometryProof,
    derivatives: Vec<(Ident, Ident)>,
    kind_helpers: std::collections::HashMap<String, String>,
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
impl<'src> Parser<'src, '_> {
    fn cur(&self) -> Lex<'src> {
        self.tokens[self.at]
    }
    fn is(&self, s: &str) -> bool {
        self.cur().text == s
    }
    fn pop(&mut self) -> Lex<'src> {
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
    fn reference(&mut self) -> Res<Ident> {
        let mut name = self.ident()?;
        if self.eat("::") {
            let member = self.ident()?;
            name.name.push_str("::");
            name.name.push_str(&member.name);
            name.span = name.span.to(member.span);
        }
        Ok(name)
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
        let n = self.reference()?;
        Ok(SpaceRef {
            var: self.generics.contains(&n.name),
            name: n.name,
            span: n.span,
        })
    }
    fn ty(&mut self) -> Res<TyExpr> {
        let span = self.cur().span;
        if self.eat("array") {
            self.expect("[")?;
            let element = self.ty()?;
            self.expect(",")?;
            let count = self.integer()?;
            let span = span.to(self.cur().span);
            self.expect("]")?;
            return Ok(TyExpr::Array(Box::new(element), count, span));
        }
        if self.eat("[") {
            let t = self.reference()?;
            self.expect("]")?;
            return Ok(TyExpr::Table(t));
        }
        if unit(self.cur().text) || self.is("1") {
            let span = self.cur().span;
            loop {
                if !unit(self.cur().text) && !self.is("1") {
                    return self.err("a unit atom");
                }
                self.pop();
                if self.eat("^") {
                    self.eat("-");
                    self.integer()?;
                }
                if !self.eat("/") && !self.eat("*") { break; }
            }
            return Ok(ty("f32", span));
        }
        let n = self.reference()?;
        if matches!(n.name.as_str(),"Pt"|"Vec"|"Dir"|"Norm"|"Xform"|"Field"){
            return Err(Diag::new(Code::E001,n.span,"Syntax A type in E+ source","write p3@Space, v3@Space, d3@Space, n3@Space or xf@A>B"))
        }
        if self.eat("@") {
            let a = self.space()?;
            return Ok(match n.name.as_str() {
                "xf" => {
                    self.expect(">")?;
                    TyExpr::Xform(a, self.space()?)
                }
                "attach" => TyExpr::Geo(Geo::Attach,a),
                "target" => TyExpr::Geo(Geo::Target,a),
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
        let name = TYPE_ALIASES.iter()
            .find_map(|&(short, full)| (n.name == short).then_some(full))
            .unwrap_or(&n.name);
        Ok(ty(name, n.span))
    }
    fn upper_ident(&mut self) -> Res<Ident> {
        if !self.cur().text.as_bytes().first().is_some_and(u8::is_ascii_uppercase) {
            return self.err("an uppercase identifier");
        }
        self.ident()
    }
    fn column(&mut self) -> Res<RowField> {
        let name = self.ident()?;
        self.expect(":")?;
        let ty = self.ty()?;
        let range = self.range()?;
        let default = if self.eat("=") { Some(self.expr(0)?) } else { None };
        Ok(RowField { name, ty, range, default })
    }
    fn resources(&mut self, keyword: &str) -> Res<Vec<Ident>> {
        let mut names = vec![];
        if self.eat(keyword) {
            self.expect("[")?;
            while !self.eat("]") {
                names.push(self.reference()?);
                if !self.eat(",") {
                    self.expect("]")?;
                    break;
                }
            }
        }
        Ok(names)
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

    fn item(&mut self, public: bool) -> Res<Item> {
        let build = self.eat("build");
        let start = self.cur().span;
        if build && self.is("{") {
            let name = named(&format!("#build_{}_{}", start.file, start.lo), start);
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
        if self.eat("struct") {
            let name = self.upper_ident()?;
            self.body_start(&name);
            self.expect("{")?;
            self.seps();
            let mut fields = vec![];
            while !self.eat("}") {
                fields.push(self.column()?);
                if !self.eat(",") { self.end()?; }
                self.seps();
            }
            return Ok(Item::Struct(name, fields));
        }
        if self.eat("type") {
            let name = self.upper_ident()?;
            self.expect("=")?;
            return Ok(Item::TypeAlias(name, self.ty()?));
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
        if self.eat("bake_tex") {
            return self.bake_tex().map(Item::Bake);
        }
        if self.eat("kind") {
            return self.kind().map(Item::Kind);
        }
        if self.eat("schedule") {
            let name = self.ident()?;
            let params = if self.is("(") { self.schedule_params()? } else { vec![] };
            // Merge (lean2 body ranges + probe-door schedule params): body_start records the next token as the
            // body's opening, so it must follow the parameter list.
            self.body_start(&name);
            self.expect("{")?;
            self.seps();
            let mut radiance = None;
            let mut passes = vec![];
            while !self.eat("}") {
                if self.eat("type") {
                    let n = self.cur().span;
                    self.expect("Radiance")?;
                    self.expect("=")?;
                    if radiance.replace((named("Radiance", n), self.ty()?)).is_some() {
                        return Err(Diag::new(Code::E011, n, "duplicate Radiance binding", "bind Radiance once"));
                    }
                    self.end()?;
                    continue;
                }
                let explicit = self.eat("pass");
                let verb = self.reference()?;
                let (mut args, mut body, mut resources) = (vec![], vec![], None);
                if explicit {
                    let reads = self.resources("reads")?;
                    let writes = self.resources("writes")?;
                    resources = Some(PassResources { reads, writes, body: self.block(false)? });
                } else {
                    args = self.args()?;
                    if self.eat("{") {
                        self.seps();
                        while !self.eat("}") {
                            let f = self.ident()?;
                            body.push((f, self.args()?));
                            self.end()?
                        }
                    }
                }
                let clock = if self.eat("on") { Some(self.clock()?) } else { None };
                passes.push(PassDecl { label: verb.clone(), verb, call_site: None, args, body, resources, clock });
                self.end()?
            }
            return Ok(Item::Schedule(ScheduleDecl { implicit: false, name, params, passes, radiance }));
        }
        if self.eat("world") {
            let name = self.upper_ident()?;
            self.body_start(&name);
            self.expect("{")?;
            self.seps();
            self.expect("things")?;
            self.expect(":")?;
            self.expect("[")?;
            self.seps();
            let mut things = Vec::new();
            while !self.eat("]") {
                things.push(self.reference()?);
                if !self.eat(",") { self.expect("]")?; break; }
                self.seps();
            }
            self.eat(",");
            self.seps();
            self.expect("}")?;
            return Ok(Item::World(name, things));
        }
        if self.eat("camera") {
            let name = self.ident()?;
            self.body_start(&name);
            self.expect("{")?;
            let fields = self.fields("}")?;
            return Ok(Item::Camera(CameraDecl { name, fields }));
        }
        if self.eat("target") {
            return self.target(start).map(Item::Target);
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
            self.body_start(&name);
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
        if self.eat("kernel") {
            let name = self.ident()?;
            let params = self.params()?;
            let tier = if self.eat("nondet") { Determinism::NonDet } else { Determinism::Det };
            let mut allow = std::collections::BTreeSet::new();
            if self.eat("allow") {
                self.expect("(")?;
                loop {
                    let cap = self.ident()?;
                    let value = match cap.name.as_str() {
                        "atomics" => KernelCapability::Atomics,
                        "workgroup" => KernelCapability::Workgroup,
                        "subgroups" => KernelCapability::Subgroups,
                        "dynamic_loops" => KernelCapability::DynamicLoops,
                        _ => return Err(Diag::new(Code::E001, cap.span, "unknown kernel capability",
                            "use atomics, workgroup, subgroups or dynamic_loops")),
                    };
                    if !allow.insert(value) {
                        return Err(Diag::new(Code::E011, cap.span, "duplicate kernel capability", "list it once"));
                    }
                    if !self.eat(",") || self.is(")") { break; }
                }
                self.expect(")")?;
            }
            self.body_start(&name);
            let body = self.block(false)?;
            let f = FnDecl { comptime: false, attrs: vec![], name, params, ret: None, body, raw: None };
            return Ok(Item::Kernel(KernelDecl { f, tier, allow, writes: Default::default() }));
        }
        if self.eat("law") {
            let name = self.ident()?;
            let params = self.params()?;
            let tier = if self.eat("nondet") { Determinism::NonDet } else { Determinism::Det };
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
            self.body_start(&name);
            let body = self.block(false)?;
            return Ok(Item::Law(LawDecl {
                tier,
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
            let body = self.named_body(&name, true)?;
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
            let binding = loop {
                if !self.eat("::") {
                    break ImportBinding::Module(self.eat("as").then(|| self.ident()).transpose()?);
                }
                if self.eat("*") {
                    break ImportBinding::Wildcard;
                }
                if self.eat("{") {
                    let mut names = vec![self.ident()?];
                    while self.eat(",") && !self.is("}") {
                        names.push(self.ident()?);
                    }
                    self.expect("}")?;
                    break ImportBinding::Selected(names);
                }
                path.push(self.ident()?);
            };
            let span = start.to(self.tokens[self.at - 1].span);
            return Ok(Item::Use(Import { path, binding, span }));
        }
        Err(Diag::new(
            Code::E404,
            start,
            format!("unsupported module item `{}`", self.cur().text),
            "declare a v1 item or put row generation inside build { ... }",
        ))
    }
    fn target(&mut self, start: Span) -> Res<TargetDecl> {
        let name = self.upper_ident()?;
        self.expect(":")?;
        let format = self.ident()?;
        let extent = if self.eat("view") {
            let scale = if self.eat("*") {
                let token = self.cur();
                let scale = token.text.parse().map_err(|_| Diag::new(
                    Code::E001, token.span, "invalid target scale", "use a decimal number"))?;
                self.pop();
                scale
            } else {
                1.0
            };
            TargetExtent::View(scale)
        } else if self.cur().kind == Kind::Grid {
            let token = self.pop();
            let (width, height) = token.text.split_once('x').expect("grid token");
            let dimension = |text: &str| text.parse::<u32>().map_err(|_| Diag::new(
                Code::E001, token.span, "invalid target dimensions", "write two integer sides as WxH"));
            TargetExtent::Fixed(dimension(width)?, dimension(height)?)
        } else if self.cur().kind == Kind::Number {
            let width = self.integer()?;
            self.expect("x")?;
            TargetExtent::Fixed(width, self.integer()?)
        } else {
            TargetExtent::View(1.0)
        };
        let samples = if self.eat("msaa") { self.integer()? } else { 1 };
        let span = start.to(self.tokens[self.at - 1].span);
        Ok(TargetDecl { name, format, extent, samples, span })
    }

    fn bake_tex(&mut self) -> Res<BakeDecl> {
        let name = self.upper_ident()?;
        let cube = self.eat("cube");
        let grid = self.cur();
        let size = grid.text.split('x').map(str::parse::<u32>).collect::<Result<Vec<_>, _>>();
        let err = || Diag::new(Code::E001, grid.span, "invalid bake dimensions", "use WxH, WxHxD or cube NxN with positive sides");
        let size = size.map_err(|_| err())?;
        let dim = match (cube, size.as_slice()) {
            (true, [w, h]) if w == h && *w > 0 => TexDim::Cube(*w),
            (false, [w, h]) if *w > 0 && *h > 0 => TexDim::D2(*w, *h),
            (false, [w, h, d]) if *w > 0 && *h > 0 && *d > 0 => TexDim::D3(*w, *h, *d),
            _ => return Err(err()),
        };
        self.pop();
        let format = self.ident()?;
        let ret = match format.name.as_str() {
            "r8unorm" | "r16float" | "r32float" => "f32",
            "rgba8unorm" | "rgba16float" => "vec4f",
            _ => return Err(Diag::new(Code::E405, format.span, "unsupported bake format", "use r8unorm, r16float, r32float, rgba8unorm or rgba16float")),
        };
        self.expect("on")?;
        let clock = self.clock()?;
        self.expect("mips")?;
        self.expect("(")?;
        let mips = if self.eat("true") { true } else { self.expect("false")?; false };
        self.expect(")")?;
        let params = self.params()?;
        if params.len() != 1 || params[0].ty.is_none() {
            return Err(Diag::new(Code::E003, name.span, "bake needs one typed coordinate", "use v2, v3 or d3@World for a 2D, 3D or cube bake"));
        }
        self.locals = params.iter().map(|p| p.name.name.clone()).collect();
        self.body_start(&name);
        let body = self.block(true)?;
        self.locals.clear();
        let texel = FnDecl { comptime: false, attrs: vec![], ret: Some(ty(ret, format.span)), name, params, body, raw: None };
        Ok(BakeDecl { texel, dim, format, clock, mips })
    }

    /// After `on`: `load` or `dirty(name, ...)`.
    fn clock(&mut self) -> Res<BakeClock> {
        if self.eat("load") {
            return Ok(BakeClock::Load);
        }
        self.expect("dirty")?;
        self.expect("(")?;
        let mut names = vec![self.reference()?];
        while self.eat(",") {
            names.push(self.reference()?);
        }
        self.expect(")")?;
        Ok(BakeClock::Dirty(names))
    }

    fn schedule_params(&mut self) -> Res<Vec<ScheduleParam>> {
        self.expect("(")?;
        let mut out = vec![];
        while !self.eat(")") {
            if self.eat("..") {
                out.push(ScheduleParam::Rest(self.ident()?));
            } else {
                let name = self.ident()?;
                let default = if self.eat("=") { Some(self.expr(0)?) } else { None };
                out.push(ScheduleParam::One { name, default });
            }
            if !self.eat(",") {
                self.expect(")")?;
                break;
            }
        }
        Ok(out)
    }

    fn params(&mut self) -> Res<Vec<Param>> {
        self.parameter_list(false)
    }
    fn parameter_list(&mut self, build: bool) -> Res<Vec<Param>> {
        self.expect("(")?;
        let mut out = vec![];
        while !self.eat(")") {
            let name = self.ident()?;
            let ty = if self.eat(":") {
                Some(self.ty()?)
            } else {
                None
            };
            let default = if build && self.eat("=") { Some(self.expr(0)?) } else { None };
            if build && out.iter().any(|p: &Param| p.name.name == name.name) {
                return Err(Diag::new(Code::E003, name.span, "duplicate build parameter", "give each parameter its own name"));
            }
            out.push(Param { name, ty, default });
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
        let params = self.parameter_list(build)?;
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
        let body = self.named_body(&n, !build || ret.is_some())?;
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
        // A kind without fields may omit `()`: `kind Sand @World { ... }` (2 of 13 fresh crab programs).
        let fieldless = self.is("{");
        if !fieldless {
            self.expect("(")?;
        }
        let mut row = vec![];
        while !fieldless && !self.eat(")") {
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
        // A kind parameter is also a column, so CPU evaluation, tables and hosts read it from the
        // row as before; only the GPU modules fold it to a constant (one compiled variant).
        row.extend(params.iter().map(|prm: &KindParam| RowField {
            name: prm.name.clone(),
            ty: prm.ty.clone(),
            range: None,
            default: None,
        }));
        self.fields = row.iter().map(|f| f.name.name.clone()).collect();
        self.body_start(&name);
        self.expect("{")?;
        self.seps();
        self.proof = GeometryProof::Required;
        let fns = vec![];
        let mut queries = vec![];
        let mut maps = vec![];
        let start=self.at;
        let mut nested=0usize;
        while !self.is("") {
            if self.is("}") && nested==0{break}
            if nested == 0 && self.eat("fn") {
                let n = self.ident()?;
                if self.kind_helpers.insert(n.name.clone(), format!("{}#{}", name.name, n.name)).is_some() {
                    return Err(Diag::new(Code::E011, n.span, "duplicate kind helper", "declare one helper with this name"));
                }
                continue;
            }
            if nested==0 && self.eat("map"){
                let name=self.ident()?;self.expect("(")?;let param=self.ident()?;self.expect(")")?;self.expect("=")?;
                let pos=self.at;maps.push((name,param,pos));continue;
            }
            match self.pop().text{"{"=>nested+=1,"}"=>nested-=1,_=>{}}
        }
        self.at=start;
        while !self.eat("}") {
            if self.eat("map") {
                let n = self.ident()?;
                self.expect("(")?;
                self.ident()?;
                self.expect(")")?;
                self.expect("=")?;
                self.map(n)?;
            } else if self.eat("fn") {
                let mut f = self.function(false, false, false)?;
                f.name.name = self.kind_helpers[&f.name.name].clone();
                // Lift the lexically resolved declaration once. All passes use the same symbol.
                self.module.items.push(Item::Fn(f));
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
                self.geometry = matches!(q, QueryKind::Trace | QueryKind::Surface);
                let mut b = self.named_body(&n, true)?;
                self.geometry = false;
                pre.append(&mut b.stmts);
                b.stmts = pre;
                ps.insert(
                    0,
                    Param {
                        name: named("self", n.span),
                        ty: None,
                        default: None,
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
        self.kind_helpers.clear();
        Ok(KindDecl {
            proof: self.proof,
            name,
            params,
            row,
            queries,
            fns,
        })
    }
    fn body_start(&mut self, name: &Ident) {
        let open = self.cur().span;
        self.module.bodies.insert(name.span, open);
    }

    fn named_body(&mut self, name: &Ident, value: bool) -> Res<Block> {
        let start = self.cur().span;
        let body = self.body(value)?;
        self.module.bodies.insert(name.span, start.to(self.tokens[self.at - 1].span));
        Ok(body)
    }

    fn body(&mut self, value: bool) -> Res<Block> {
        let eq = self.eat("=");
        if self.is("{") {
            return self.block(value);
        }
        if !eq {
            return self.err("= expression or a block");
        }
        if self.eat("if") {
            return Ok(Block { stmts: vec![self.if_stmt(value)?] });
        }
        // `trace(p) =` then `let` lines and a final expression: a block without braces. No expression starts
        // with `let`, so it reads unambiguously; fresh crab programs wrote it (`let local = p - at`, `let op`).
        if value && (self.is("let") || self.is("var")) {
            let (locals, maps) = (self.locals.clone(), self.maps.clone());
            let mut stmts = vec![];
            while self.is("let") || self.is("var") {
                stmts.push(self.stmt()?);
                self.expect(";")?;
                self.seps();
            }
            let e = self.expr(0)?;
            stmts.push(Stmt::Return(Some(e.clone()), e.span));
            self.locals = locals;
            self.maps = maps;
            return Ok(Block { stmts });
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
        let kind = self.reference()?;
        self.expect("(")?;
        let fields = self.fields(")")?;
        Ok(Stmt::Emit {
            kind,
            fields,
            span,
            bind,
        })
    }
    fn loop_body(&mut self, var: &Ident) -> Res<Block> {
        // The binder shadows a row column only after the range/list has been parsed.
        let fresh = self.locals.insert(var.name.clone());
        let body = self.block(false);
        if fresh {
            self.locals.remove(&var.name);
        }
        body
    }
    fn if_stmt(&mut self, value: bool) -> Res<Stmt> {
        let cond = self.expr(0)?;
        let then = self.block(value)?;
        let otherwise = if self.eat("else") {
            Some(if self.eat("if") {
                Block { stmts: vec![self.if_stmt(value)?] }
            } else {
                self.block(value)?
            })
        } else if value {
            return self.err("an else branch");
        } else {
            None
        };
        Ok(Stmt::If { cond, then, otherwise })
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
        if self.eat("import_table") {
            self.expect("(")?;
            if self.cur().kind != Kind::String {
                return self.err("a binary table path");
            }
            let path = self.pop().text.to_string();
            self.expect(",")?;
            let kind = self.reference()?;
            self.expect(",")?;
            let index = self.expr(0)?;
            self.expect(")")?;
            return Ok(Stmt::ImportTable { kind, path, index, span: sp });
        }
        let build = self.eat("build");
        if build || self.is("for") {
            self.expect("for")?;
            let var = self.ident()?;
            self.expect("in")?;
            if build && self.eat("[") {
                let values = self.list("]")?.into_iter().map(|arg| arg.value).collect();
                let body = self.loop_body(&var)?;
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
            let body = self.loop_body(&var)?;
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
            return self.if_stmt(false);
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
                        "stretch"=>{a=self.num(1.,true,s);continue;}
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
                if self.is("(") {
                    let mut args = self.args()?;
                    args.insert(0, Arg { name: None, value: a });
                    // A receiver call carries a distinct internal symbol from an ordinary function.
                    m.name.insert(0, '.');
                    a = self.node(s.to(self.tokens[self.at - 1].span), ExprKind::Call { callee: m, args });
                    continue;
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
    fn expr_arm(&mut self) -> Res<ExprBlock> {
        let mut body = self.block(false)?;
        let value = match body.stmts.pop() {
            Some(Stmt::Expr(e) | Stmt::Return(Some(e), _)) => e,
            _ => return self.err("a value at the end of the block"),
        };
        Ok(ExprBlock { body, value: Box::new(value) })
    }
    fn match_arm(&mut self) -> Res<((i64, Span), Expr)> {
        let start = self.cur().span;
        let negative = self.eat("-");
        let token = self.cur();
        let invalid = || Diag::new(Code::E001, token.span, "expected an integer match pattern",
            "write a signed or unsigned integer literal");
        if token.kind != Kind::Number {
            return Err(invalid());
        }
        self.pop();
        let n = if let Some(hex) = token.text.strip_prefix("0x") {
            u32::from_str_radix(hex, 16)
        } else {
            token.text.strip_suffix('u').or_else(|| token.text.strip_suffix('i'))
                .unwrap_or(token.text).parse::<u32>()
        }.map_err(|_| invalid())?;
        let key = if negative { -i64::from(n) } else { i64::from(n) };
        self.expect("=>")?;
        self.seps();
        Ok(((key, start.to(token.span)), self.expr(0)?))
    }

    fn match_expr(&mut self, start: Span) -> Res<Expr> {
        let condition = Box::new(self.expr(0)?);
        self.expect("{")?;
        self.seps();
        let (first, then) = self.match_arm()?;
        self.expect(",")?;
        let mut rest = vec![];
        while !self.eat("_") {
            rest.push(self.match_arm()?);
            self.expect(",")?;
        }
        self.expect("=>")?;
        self.seps();
        let otherwise = self.expr(0)?;
        self.eat(",");
        self.seps();
        let end = self.cur().span;
        self.expect("}")?;
        let arm = |value| ExprBlock { body: Block { stmts: vec![] }, value: Box::new(value) };
        Ok(self.node(start.to(end), ExprKind::If {
            condition, then: arm(then), otherwise: arm(otherwise), cases: Some(MatchCases { first, rest }),
        }))
    }

    fn primary(&mut self) -> Res<Expr> {
        let span = self.cur().span;
        if self.cur().kind == Kind::String {
            let literal = self.pop();
            return Ok(self.node(span, ExprKind::String(literal.text.into())));
        }
        if self.eat("unproved") {
            if !self.geometry {
                return Err(Diag::new(Code::E405, span, "unproved outside a geometry query",
                    "place the block inside trace or surface"));
            }
            self.expect("(")?;
            if self.is("step_scale") && self.tokens.get(self.at + 1).is_some_and(|t| t.text == ":") {
                self.pop();
                self.pop();
            }
            let step_scale = Box::new(self.expr(0)?);
            self.expect(")")?;
            let outer = std::mem::replace(&mut self.in_opt_out, true);
            let body = self.expr_arm()?;
            self.in_opt_out = outer;
            self.proof = GeometryProof::Unproved(UnprovedReason::AuthorOptOut);
            return Ok(self.node(span.to(body.value.span), ExprKind::Unproved { step_scale, body }));
        }
        if self.eat("wgsl"){
            let params=self.params()?;
            if let Some(p)=params.iter().find(|p|p.ty.is_none()){return Err(Diag::new(Code::E003,p.name.span,"WGSL capture requires a type","annotate the boundary parameter"))}
            self.expect("->")?;let ret=self.ty()?;
            if self.cur().kind!=Kind::Raw{return self.err("a balanced WGSL block")}
            let island=self.pop();let end=island.span;
            let raw=RawWgsl { body: island.text.into(), references: vec![] };
            let name=named(&format!("#raw_{}_{}",span.file,span.lo),span);
            let args=params.iter().map(|prm|Arg{name:None,value:self.node(prm.name.span,ExprKind::Name(prm.name.name.clone()))}).collect();
            self.module.items.push(Item::Fn(FnDecl{name:name.clone(),params,ret:Some(ret),body:Block{stmts:vec![]},raw:Some(raw),attrs:vec![],comptime:false}));
            return Ok(self.node(span.to(end),ExprKind::Call{callee:name,args}));
        }
        if self.eat("if") {
            let condition = Box::new(self.expr(0)?);
            let then = self.expr_arm()?;
            self.expect("else")?;
            let otherwise = if self.is("if") {
                ExprBlock { body: Block { stmts: vec![] }, value: Box::new(self.primary()?) }
            } else {
                self.expr_arm()?
            };
            return Ok(self.node(span, ExprKind::If { condition, then, otherwise, cases: None }));
        }
        if self.eat("match") {
            return self.match_expr(span);
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
        let mut n = self.reference()?;
        if n.name == "true" || n.name == "false" {
            return Ok(self.node(span, ExprKind::Bool(n.name == "true")));
        }
        if self.eat("'") || n.name == "diff" && self.is("(") {
            let (f, x) = if n.name == "diff" {
                self.expect("(")?;
                let f = self.reference()?;
                self.expect(",")?;
                let x = self.ident()?;
                self.expect(")")?;
                (f, x)
            } else {
                (n.clone(), named("", span))
            };
            if self.in_opt_out {
                let args = self.args()?;
                return Ok(self.node(span, ExprKind::FiniteDiff { function: f, parameter: x, args }));
            }
            n.name = format!("{}__d_{}", f.name, x.name);
            self.derivatives.push((f, x));
        }
        if self.is("(") {
            let mut args = self.args()?;
            if matches!(n.name.as_str(), "p3" | "v3") && self.eat("@") {
                if args.is_empty() || args.iter().any(|arg| arg.name.is_some()) {
                    return self.err("positional point or vector components");
                }
                let mut space = self.space()?;
                space.var |= self.maps.contains(&space.name);
                let span = span.to(space.span);
                let geo = if n.name == "p3" { Geo::Pt } else { Geo::Vec };
                return Ok(self.node(span, ExprKind::Ctor { ty: TyExpr::Geo(geo, space), args }));
            }
            if self
                .module
                .items
                .iter()
                .any(|i| matches!(i,Item::Scan(sc) if sc.name.name==n.name))
            {
                let scan = self.node(span, ExprKind::Name(n.name.clone()));
                args.insert(
                    0,
                    Arg {
                        name: None,
                        value: scan,
                    },
                );
                n.name = "sample_c2".into();
            }
            if let Some(resolved) = self.kind_helpers.get(&n.name) {
                n.name = resolved.clone();
                return Ok(self.node(span, ExprKind::Call { callee: n, args }));
            }
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
        if self.maps.contains(&n.name) && !self.is(".") {return Err(Diag::new(Code::E106,n.span,"a map is not a point","use its .p, .pt(...), .in(...) or .out(...) member"))}
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
        // WGSL's integer suffixes: `0u`, `3i`. The literal stays an integer typed by its context, as `0` is.
        if let Some(int) = s.strip_suffix('u').or_else(|| s.strip_suffix('i'))
            && !int.is_empty() && int.bytes().all(|b| b.is_ascii_digit())
        {
            return Ok(self.num(int.parse::<u32>().map_err(|_| bad())?.into(), false, sp));
        }
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
                let exponent = power.checked_mul(op).ok_or_else(bad)?;
                value *= factor.powi(exponent);
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
pub const TYPE_ALIASES: &[(&str, &str)] = &[
    ("f", "f32"), ("u", "u32"), ("i", "i32"),
    ("v2", "vec2f"), ("rgb", "vec3f"), ("v3", "vec3f"), ("v4", "vec4f"),
];

pub const UNIT_SYMBOLS: &[&str] = &["m", "mm", "rad", "deg", "kg", "s", "N", "J", "Hz"];

fn unit(s: &str) -> bool {
    UNIT_SYMBOLS.contains(&s)
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
