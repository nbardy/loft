//! WGSL emit (plan §1 layer 2): one module per kind, never per instance, and three shared modules (cull, classify,
//! present). A kind's text depends only on its declaration and the library fns it reaches; its id, row base and row
//! count are data in `KindParams`. The author's structure is kept: their fns stay fns, their loops stay loops.

use crate::Prog;
#[path="motion.rs"]
mod motion;
pub use motion::motion_module;
use loft_ir::ast::*;
use loft_ir::program::{abi, Draw};
use loft_ir::{Code, Diag, Res};
use std::collections::BTreeSet;
use std::fmt::Write;

const PRELUDE: &str = r"struct Frame {
  view_proj: mat4x4<f32>,
  inv_view_proj: mat4x4<f32>,
  eye: vec4<f32>,
  sun: vec4<f32>,
  size: vec4<u32>,
  cold: vec4<u32>,
  light_vp: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> frame: Frame;
struct LoftEnvironment { sun_color: vec4<f32>, zenith: vec4<f32>, horizon: vec4<f32> }
@group(0) @binding(2) var<storage, read> environment: LoftEnvironment;
const LOFT_NONCE: u32 = {{NONCE}};
const LOFT_EMPTY: u32 = 0xffffffffu;

struct LoftXform { r: mat3x3<f32>, t: vec3<f32> }
fn loft_xf_pt(x: LoftXform, p: vec3<f32>) -> vec3<f32> { return x.r * p + x.t; }
fn loft_xf_vec(x: LoftXform, v: vec3<f32>) -> vec3<f32> { return x.r * v; }
fn loft_xf_inv(x: LoftXform) -> LoftXform { let rt = transpose(x.r); return LoftXform(rt, -(rt * x.t)); }
fn loft_xf_mul(a: LoftXform, b: LoftXform) -> LoftXform { return LoftXform(a.r * b.r, a.r * b.t + a.t); }
// Rotations of the plane (i, j): i' = cos i + sin j, j' = cos j - sin i (the evaluator's rule).
fn loft_rot_x(p: vec3<f32>, a: f32) -> vec3<f32> {
  let s = sin(a);
  let c = cos(a);
  return vec3<f32>(p.x, c * p.y + s * p.z, c * p.z - s * p.y);
}
fn loft_rot_y(p: vec3<f32>, a: f32) -> vec3<f32> {
  let s = sin(a);
  let c = cos(a);
  return vec3<f32>(c * p.x + s * p.z, p.y, c * p.z - s * p.x);
}
fn loft_rot_z(p: vec3<f32>, a: f32) -> vec3<f32> {
  let s = sin(a);
  let c = cos(a);
  return vec3<f32>(c * p.x + s * p.y, c * p.y - s * p.x, p.z);
}
fn loft_unit(e0: f32, e1: f32, x: f32) -> f32 { return clamp((x - e0) / (e1 - e0), 0.0, 1.0); }
fn loft_smootherstep(e0: f32, e1: f32, x: f32) -> f32 { let t = loft_unit(e0, e1, x); return t * t * t * ((6.0 * t - 15.0) * t + 10.0); }
fn loft_dsmoothstep(e0: f32, e1: f32, x: f32) -> f32 { let t = loft_unit(e0, e1, x); return 6.0 * t * (1.0 - t) / (e1 - e0); }
fn loft_dsmootherstep(e0: f32, e1: f32, x: f32) -> f32 { let t = loft_unit(e0, e1, x); return 30.0 * t * t * (1.0 - t) * (1.0 - t) / (e1 - e0); }
struct LoftFrame { origin: vec3<f32>, tangent: vec3<f32>, normal: vec3<f32>, binormal: vec3<f32> }
// The camera ray through a pixel position (pixel units, centre at +0.5).
fn loft_ray(px: vec2<f32>) -> vec3<f32> {
  let ndc = vec2<f32>(px.x / f32(frame.size.x) * 2.0 - 1.0, 1.0 - px.y / f32(frame.size.y) * 2.0);
  let far = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
  return normalize(far.xyz / far.w - frame.eye.xyz);
}
";

/// Lowers a type for WGSL: spaces and units erase to their storage.
fn wty(t: &Ty) -> String {
    match t {
        Ty::Void => String::new(),
        Ty::Bool => "bool".into(),
        Ty::U32 => "u32".into(),
        Ty::I32 | Ty::AbstractInt => "i32".into(),
        Ty::F32 => "f32".into(),
        Ty::VecN(n) => format!("vec{n}<f32>"),
        Ty::Geo(..) => "vec3<f32>".into(),
        Ty::Xform(..) => "LoftXform".into(),
        Ty::Row(_) => "Row".into(),
        Ty::Frame(_) => "LoftFrame".into(),
        Ty::Bsdf => "Bsdf".into(),
        Ty::Id(_) => "u32".into(),
        Ty::Table(_) | Ty::Verdict => unreachable!("motion values are lowered in a law"),
        Ty::Map(..) => unreachable!("maps are not values"),
    }
}

// Escape the escape prefix as well, so a user declaration cannot collide
// with the encoded spelling of a WGSL keyword. Row fields have their own prefix.
fn wgsl_name(n:&str)->String{
    if naga::keywords::wgsl::RESERVED.contains(&n) || n.starts_with("loft_user_") || matches!(n,"frame"|"rows"|"visible"|"kp"|"luts"|"self_"|"load_row"){
        format!("loft_user_{n}")
    }else{n.into()}
}

fn f32_lit(v: f64) -> String {
    let s = format!("{:?}", v as f32);
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") { s } else { format!("{s}.0") }
}

/// A map statement's emitted locals, for `.cell`, `.to_parent` and `.from_parent`.
#[derive(Clone)]
enum MapStage {
    Mirror,
    Translate(String),
    /// Axis letter and the angle's local.
    Rot(char, String),
}

/// A baked scan's place in the program's table buffer (frames of 12 words).
pub struct LutRef {
    pub name: String,
    pub offset: u32,
    pub n: u32,
    pub lo: f32,
    pub hi: f32,
}

pub struct Emitter<'p, 'a> {
    p: &'p Prog<'a>,
    types: &'p [Ty],
    out: String,
    maps: Vec<(String, Vec<MapStage>)>,
    luts: &'p [LutRef],
    motion:bool,
    environment:[bool;2],
}

/// The lobe calls of a `Bsdf` sum, bottom first.
fn lobes<'e>(e: &'e Expr, out: &mut Vec<(&'e str, &'e [Arg])>) -> bool {
    match &e.kind {
        ExprKind::Binary(BinOp::Add, a, b) => lobes(a, out) && lobes(b, out),
        ExprKind::Call { callee, args } => {
            out.push((&callee.name, args));
            true
        }
        ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => false,
    }
}

/// The fns, user and library, reachable from a kind's queries and the shade glue's light fn.
pub(crate) fn reachable<'a>(p: &Prog<'a>, roots: &[&'a FnDecl], extra: &[&str]) -> (Vec<&'a FnDecl>, BTreeSet<String>) {
    fn walk_expr(e: &Expr, calls: &mut Vec<String>, names: &mut Vec<String>) {
        match &e.kind {
            ExprKind::Call { callee, args } => {
                calls.push(callee.name.clone());
                args.iter().for_each(|a| walk_expr(&a.value, calls, names));
            }
            ExprKind::Ctor { args, .. } => args.iter().for_each(|a| walk_expr(&a.value, calls, names)),
            ExprKind::Name(n) => names.push(n.clone()),
            ExprKind::Member(b, _) | ExprKind::Unary(_, b) => walk_expr(b, calls, names),
            ExprKind::Index(a, b) | ExprKind::Binary(_, a, b) => {
                walk_expr(a, calls, names);
                walk_expr(b, calls, names);
            }
            ExprKind::Num { .. } | ExprKind::Bool(_) => {}
        }
    }
    fn walk_block(b: &Block, calls: &mut Vec<String>, names: &mut Vec<String>) {
        for s in &b.stmts {
            match s {
                Stmt::Let { value, .. } => walk_expr(value, calls, names),
                Stmt::Assign { target, value, .. } => {
                    walk_expr(target, calls, names);
                    walk_expr(value, calls, names);
                }
                Stmt::Map { head, stages, .. } => {
                    walk_expr(head, calls, names);
                    stages.iter().flat_map(|(_, a)| a).for_each(|a| walk_expr(a, calls, names));
                }
                Stmt::Return(v, _) => v.iter().for_each(|e| walk_expr(e, calls, names)),
                Stmt::If { cond, then, otherwise } => {
                    walk_expr(cond, calls, names);
                    walk_block(then, calls, names);
                    otherwise.iter().for_each(|o| walk_block(o, calls, names));
                }
                Stmt::Each {values,body,..}=>{for e in values{walk_expr(e,calls,names);}walk_block(body,calls,names);}
                Stmt::For { lo, hi, body, .. } => {
                    walk_expr(lo, calls, names);
                    walk_expr(hi, calls, names);
                    walk_block(body, calls, names);
                }
                Stmt::Emit { fields, .. } => fields.iter().for_each(|(_, e)| walk_expr(e, calls, names)),
                Stmt::Expr(e) => walk_expr(e, calls, names),
            }
        }
    }
    let mut seen: Vec<&FnDecl> = vec![];
    let mut consts = BTreeSet::new();
    let mut todo: Vec<&FnDecl> = roots.to_vec();
    for f in extra.iter().filter_map(|n| p.fns.get(n).copied()) {
        seen.push(f);
        todo.push(f);
    }
    while let Some(f) = todo.pop() {
        let (mut calls, mut names) = (vec![], vec![]);
        walk_block(&f.body, &mut calls, &mut names);
        while let Some(n)=names.pop(){
            if let Some(c)=p.consts.get(n.as_str()) && consts.insert(n){
                walk_expr(&c.value,&mut calls,&mut names);
            }
        }
        for c in calls {
            if let Some(g) = p.fns.get(c.as_str())
                && !seen.iter().any(|s| std::ptr::eq(*s, *g))
                && !roots.iter().any(|s| std::ptr::eq(*s, *g))
            {
                seen.push(g);
                todo.push(g);
            }
        }

    }
    // Callees before callers: WGSL allows any order, but readers expect it.
    seen.reverse();
    (seen, consts)
}

impl<'p, 'a> Emitter<'p, 'a> {
    pub fn new(p: &'p Prog<'a>, types: &'p [Ty]) -> Self {
        Emitter { p, types, out: String::new(), maps: vec![], luts: &[],motion:false,environment:[false;2] }
    }

    fn ty_of(&self, e: &Expr) -> &Ty {
        &self.types[e.id as usize]
    }

    fn resolve(&self, t: &TyExpr) -> Ty {
        // Signature types never mention a map space, so erasure needs no scope.
        match t {
            TyExpr::Name(i) => match i.name.as_str() {
                "u32" => Ty::U32,
                "i32" => Ty::I32,
                "bool" => Ty::Bool,
                "vec2f" => Ty::VecN(2),
                "vec3f" => Ty::VecN(3),
                "vec4f" => Ty::VecN(4),
                "Bsdf" => Ty::Bsdf,
                _ => Ty::F32,
            },
            TyExpr::Generic(i, _) => Ty::VecN(i.name.as_bytes()[3] - b'0'),
            TyExpr::Geo(g, _) => Ty::Geo(*g, Space::Var(String::new())),
            TyExpr::Xform(..) => Ty::Xform(Space::Var(String::new()), Space::Var(String::new())),
            TyExpr::Table(n) => Ty::Table(n.name.clone()),
            TyExpr::Field(_) => Ty::F32,
            TyExpr::Frame(_) => Ty::Frame(Space::Var(String::new())),
        }
    }

    fn func(&mut self, f: &FnDecl, name: &str) {
        let params: Vec<String> = f
            .params
            .iter()
            .map(|prm| match &prm.ty {
                Some(TyExpr::Name(n)) if self.p.kinds.contains_key(n.name.as_str())=>format!("{}: Row",wgsl_name(&prm.name.name)),
                Some(t) => format!("{}: {}", wgsl_name(&prm.name.name), wty(&self.resolve(t))),
                None => "self_: Row".into(),
            })
            .collect();
        let ret = f.ret.as_ref().map(|t| format!(" -> {}", wty(&self.resolve(t)))).unwrap_or_default();
        let _ = writeln!(self.out, "fn {name}({}){ret} {{", params.join(", "));
        if let Some(raw)=&f.raw{
            self.out.push_str(&crate::provenance::raw(f.name.span,&raw.body[1..raw.body.len()-1]));self.out.push_str("\n}\n");return;
        }
        self.maps.clear();
        self.block_body(&f.body, 1);
        self.out += "}\n\n";
    }

    fn block_body(&mut self, b: &Block, depth: usize) {
        let mark = self.maps.len();
        for s in &b.stmts {
            self.stmt(s, depth);
        }
        self.maps.truncate(mark);
    }

    fn line(&mut self, depth: usize, s: &str) {
        let _ = writeln!(self.out, "{}{s}", "  ".repeat(depth));
    }

    fn stmt(&mut self, s: &Stmt, d: usize) {
        match s {
            Stmt::Let { mutable, name, value, .. } => {
                let v = self.expr(value);
                let t = wty(self.ty_of(value));
                let kw = if *mutable { "var" } else { "let" };
                self.line(d, &format!("{kw} {}: {t} = {v};", wgsl_name(&name.name)));
            }
            Stmt::Assign { target, op, value } => {
                let (t, v) = (self.expr(target), self.expr(value));
                let op = op.map_or("=".to_string(), |o| format!("{}=", o.wgsl()));
                self.line(d, &format!("{t} {op} {v};"));
            }
            Stmt::Map { name, head, stages } => {
                let n = &wgsl_name(&name.name);
                let (point, mut all): (String, Vec<(&str, Vec<String>)>) = match &head.kind {
                    ExprKind::Call { callee, args } if crate::check::MAP_OPS.contains(&callee.name.as_str()) => {
                        (self.expr(&args[0].value), vec![(callee.name.as_str(), vec![])])
                    }
                    ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Call { .. } | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => (self.expr(head), vec![]),
                };
                for (op, args) in stages {
                    let a = args.iter().map(|x| self.expr(x)).collect();
                    all.push((op.name.as_str(), a));
                }
                self.line(d, &format!("let {n}_s0: vec3<f32> = {point};"));
                let mut recorded = vec![];
                for (i, (op, args)) in all.iter().enumerate() {
                    let (prev, cur) = (format!("{n}_s{i}"), format!("{n}_s{}", i + 1));
                    match *op {
                        "mirror_x" => {
                            self.line(d, &format!("let {n}_cell: f32 = select(-1.0, 1.0, {prev}.x >= 0.0);"));
                            self.line(d, &format!("let {cur}: vec3<f32> = vec3<f32>(abs({prev}.x), {prev}.yz);"));
                            recorded.push(MapStage::Mirror);
                        }
                        "translate" => {
                            self.line(d, &format!("let {n}_t{i}: vec3<f32> = {};", args[0]));
                            self.line(d, &format!("let {cur}: vec3<f32> = {prev} + {n}_t{i};"));
                            recorded.push(MapStage::Translate(format!("{n}_t{i}")));
                        }
                        "rot_x" | "rot_y" | "rot_z" => {
                            let axis = op.chars().last().expect("rot_?");
                            self.line(d, &format!("let {n}_a{i}: f32 = {};", args[0]));
                            self.line(d, &format!("let {cur}: vec3<f32> = loft_rot_{axis}({prev}, {n}_a{i});"));
                            recorded.push(MapStage::Rot(axis, format!("{n}_a{i}")));
                        }
                        other => unreachable!("checked map stage {other}"),
                    }
                }
                self.line(d, &format!("let {n}: vec3<f32> = {n}_s{};", all.len()));
                self.maps.push((n.clone(), recorded));
            }
            Stmt::Return(v, _) => {
                let v = v.as_ref().map(|e| format!(" {}", self.expr(e))).unwrap_or_default();
                self.line(d, &format!("return{v};"));
            }
            Stmt::If { cond, then, otherwise } => {
                let c = self.expr(cond);
                self.line(d, &format!("if {c} {{"));
                self.block_body(then, d + 1);
                if let Some(o) = otherwise {
                    self.line(d, "} else {");
                    self.block_body(o, d + 1);
                }
                self.line(d, "}");
            }
            Stmt::For { var, lo, hi, body, .. } => {
                let t = wty(self.ty_of(lo)).replace("i32", match self.ty_of(hi) {
                    Ty::U32 => "u32",
                    Ty::Void | Ty::Bool | Ty::I32 | Ty::F32 | Ty::AbstractInt | Ty::VecN(_) | Ty::Geo(..) | Ty::Xform(..) | Ty::Map(..) | Ty::Row(_) | Ty::Frame(_) | Ty::Table(_) | Ty::Verdict | Ty::Id(_) | Ty::Bsdf => "i32",
                });
                let (a, b) = (self.expr(lo), self.expr(hi));
                let v = wgsl_name(&var.name);
                self.line(d, &format!("for (var {v}: {t} = {a}; {v} < {b}; {v}++) {{"));
                self.block_body(body, d + 1);
                self.line(d, "}");
            }
            Stmt::Each{..} | Stmt::Emit { .. } => unreachable!("checked: stage 0 only, never emitted"),
            Stmt::Expr(e) => {
                let v = self.expr(e);
                if matches!(e.kind, ExprKind::Call { .. }) && *self.ty_of(e) == Ty::Void {
                    self.line(d, &format!("{v};"));
                } else {
                    self.line(d, &format!("_ = {v};"));
                }
            }
        }
    }

    fn map_apply(&self, name: &str, inverse: bool, x: String, point: bool) -> String {
        let stages = &self.maps.iter().rev().find(|(n, _)| n == name).expect("checked: map in scope").1;
        let mut v = x;
        let order: Vec<&MapStage> = if inverse { stages.iter().rev().collect() } else { stages.iter().collect() };
        for st in order {
            v = match st {
                MapStage::Mirror => format!("({v} * vec3<f32>({name}_cell, 1.0, 1.0))"),
                MapStage::Translate(t) if point => format!("({v} {} {t})", if inverse { "-" } else { "+" }),
                MapStage::Translate(_) => v,
                MapStage::Rot(axis, a) => format!("loft_rot_{axis}({v}, {}{a})", if inverse { "-" } else { "" }),
            };
        }
        v
    }

    pub fn expr(&mut self,e:&Expr)->String{crate::provenance::expression(e.span,self.expression(e))}

    fn expression(&mut self, e: &Expr) -> String {
        // A Bsdf value is the struct of its lobes' bound arguments, in the module's one layout (checked per kind).
        if *self.ty_of(e) == Ty::Bsdf && !matches!(e.kind, ExprKind::Name(_)) {
            let mut ls = vec![];
            let all = lobes(e, &mut ls) && ls.iter().all(|(n, _)| self.p.fns.get(n).is_some_and(|f| f.attrs.iter().any(|a| a.name.name == "lobe")));
            assert!(all, "checked: a Bsdf expression is a sum of lobe calls or a name");
            let a: Vec<String> = ls.iter().flat_map(|(_, args)| args.iter()).map(|x| self.expr(&x.value)).collect();
            return format!("Bsdf({})", a.join(", "));
        }
        match &e.kind {
            ExprKind::Num { value, suffix, text, .. } => match self.ty_of(e) {
                Ty::U32 => format!("{}u", *value as u32),
                Ty::I32 => format!("{}i", *value as i32),
                Ty::AbstractInt => text.clone(),
                Ty::Void | Ty::Bool | Ty::F32 | Ty::VecN(_) | Ty::Geo(..) | Ty::Xform(..) | Ty::Map(..) | Ty::Row(_) | Ty::Frame(_) | Ty::Table(_) | Ty::Verdict | Ty::Id(_) | Ty::Bsdf => f32_lit(if *suffix == NumSuffix::Deg { value.to_radians() } else { *value }),
            },
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Name(n) if n == "self" => "self_".into(),
            ExprKind::Name(n) if n=="time"=>if self.motion{"loft_tick_time"}else{"frame.eye.w"}.into(),
            ExprKind::Name(n) if self.motion && self.p.consts.contains_key(n.as_str())=>format!("motion_const_{}",wgsl_name(n)),
            ExprKind::Name(n) => wgsl_name(n),
            ExprKind::Member(base, m) => {
                if let ExprKind::Name(n) = &base.kind
                    && m.name == "cell"
                    && self.maps.iter().any(|(k, _)| k == &wgsl_name(n))
                {
                    return format!("{}_cell",wgsl_name(n));
                }
                if matches!(self.ty_of(base),Ty::Frame(_)){format!("{}.{}",self.expr(base),frame_field(&m.name))}else if matches!(self.ty_of(base),Ty::Row(_)){format!("{}.r_{}", self.expr(base),m.name)}else{format!("{}.{}", self.expr(base), m.name)}
            }
            ExprKind::Index(a, i) => format!("{}[{}]", self.expr(a), self.expr(i)),
            ExprKind::Unary(op, x) => {
                let o = match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "!",
                };
                format!("({o}{})", self.expr(x))
            }
            ExprKind::Binary(op, a, b) => {
                let point = matches!(self.ty_of(b), Ty::Geo(Geo::Pt, _));
                match (self.ty_of(a).clone(), &a.kind) {
                    (Ty::Map(..), ExprKind::Member(base, m)) => {
                        let ExprKind::Name(n) = &base.kind else { unreachable!("checked map ref") };
                        let x = self.expr(b);
                        self.map_apply(&wgsl_name(n), m.name == "to_parent", x, point)
                    }
                    (Ty::Xform(..), _) if matches!(self.ty_of(b), Ty::Xform(..)) => format!("loft_xf_mul({}, {})", self.expr(a), self.expr(b)),
                    (Ty::Xform(..), _) => {
                        let f = if point { "loft_xf_pt" } else { "loft_xf_vec" };
                        format!("{f}({}, {})", self.expr(a), self.expr(b))
                    }
                    _ => format!("({} {} {})", self.expr(a), op.wgsl(), self.expr(b)),
                }
            }
            ExprKind::Ctor { args, .. } => {
                let a: Vec<String> = args.iter().map(|x| self.expr(&x.value)).collect();
                match self.ty_of(e) {
                    Ty::Geo(Geo::Dir | Geo::Norm, _) => format!("normalize(vec3<f32>({}))", a.join(", ")),
                    Ty::Xform(..) => format!("loft_xform_yaw({}, {})", a[0], a[1]),
                    t @ Ty::Void | t @ Ty::Bool | t @ Ty::U32 | t @ Ty::I32 | t @ Ty::F32 | t @ Ty::AbstractInt | t @ Ty::VecN(_) | t @ Ty::Geo(..) | t @ Ty::Map(..) | t @ Ty::Row(_) | t @ Ty::Frame(_) | t @ Ty::Table(_) | t @ Ty::Verdict | t @ Ty::Id(_) | t @ Ty::Bsdf => format!("{}({})", wty(t), a.join(", ")),
                }
            }
            ExprKind::Call { callee, args } if matches!(callee.name.as_str(), "sample_c2" | "loft_dsample") => {
                let ExprKind::Name(n) = &args[0].value.kind else { unreachable!("checked scan name") };
                let l = self.luts.iter().find(|l| &l.name == n).expect("baked scan");
                let rest: Vec<String> = args[1..].iter().map(|x| self.expr(&x.value)).collect();
                let f = if callee.name == "sample_c2" { "loft_sample" } else { "loft_dsample" };
                format!("{f}({}u, {}u, {}, {}, {})", l.offset, l.n, f32_lit(f64::from(l.lo)), f32_lit(f64::from(l.hi)), rest.join(", "))
            }
            ExprKind::Call { callee, args } if callee.name == "loft_zero" => {
                let _ = args;
                match self.ty_of(e) {
                    Ty::U32 => "0u".into(),
                    Ty::I32 | Ty::AbstractInt => "0i".into(),
                    Ty::Frame(_) => "LoftFrame(vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0))".into(),
                    Ty::F32 => "0.0".into(),
                    t @ Ty::VecN(_) | t @ Ty::Geo(..) => format!("{}(0.0)", wty(t)),
                    other @ Ty::Void | other @ Ty::Bool | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Id(_) | other @ Ty::Bsdf => unreachable!("checked zero of {other}"),
                }
            }
            ExprKind::Call { callee, args } if callee.name == "loft_vec" => self.expr(&args[0].value),
            ExprKind::Call { callee, args } => {
                let a: Vec<String> = args.iter().map(|x| self.expr(&x.value)).collect();
                if callee.name=="place"{return format!("loft_xform_yaw({}, {})",a[0],a.get(1).map(String::as_str).unwrap_or("0.0"))}
                match callee.name.as_str(){
                    "sky_zenith"=>return if self.environment[0]{"environment.zenith.xyz"}else{"vec3<f32>(0.16,0.29,0.55)"}.into(),
                    "sky_horizon"=>return if self.environment[0]{"environment.horizon.xyz"}else{"vec3<f32>(0.48,0.55,0.64)"}.into(),
                    "has_sky"=>return self.environment[0].to_string(),
                    "sun_color"=>return if self.environment[1]{"environment.sun_color.xyz"}else{"vec3<f32>(2.2,2.0,1.7)"}.into(),
                    _=>{}
                }
                if callee.name=="coords"{return a[0].clone()}
                if self.p.fns.contains_key(callee.name.as_str()) {return format!("{}{}({})",if self.motion{"motion_fn_"}else{""},wgsl_name(&callee.name),a.join(", "));}
                if self.motion && matches!(callee.name.as_str(),"sin"|"cos"|"atan2"){return format!("det_{}({})",callee.name,a.join(", "));}
                if matches!(callee.name.as_str(),"held"|"missed"){return format!("vec2<f32>({}, {})",if callee.name=="held"{"1.0"}else{"0.0"},a[0]);}
                let name = match callee.name.as_str() {
                    "frame_axes" => "LoftFrame".into(),
                    "dir" => "normalize".into(),
                    "dist"=>"distance".into(),
                    "smootherstep" => "loft_smootherstep".to_string(),
                    "inv" => "loft_xf_inv".to_string(),
                    "vec2" | "vec2f" => "vec2<f32>".into(),
                    "loft_conditional" => "select".into(),
                    "vec3" | "vec3f" => "vec3<f32>".into(),
                    "vec4" | "vec4f" => "vec4<f32>".into(),
                    other => other.into(),
                };
                format!("{name}({})", a.join(", "))
            }
        }
    }

    fn header(&mut self, nonce: &str) {
        self.out += &PRELUDE.replace("{{NONCE}}", nonce);
        self.out += "fn loft_xform_yaw(o: vec3<f32>, yaw: f32) -> LoftXform {\n  let s = sin(yaw);\n  let c = cos(yaw);\n  return LoftXform(mat3x3<f32>(vec3<f32>(c, 0.0, -s), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(s, 0.0, c)), o);\n}\n\n";
    }

    fn library(&mut self, fns: &[&FnDecl], consts: &BTreeSet<String>) {
        for c in consts {
            let decl = self.p.consts[c.as_str()];
            let v = self.expr(&decl.value);
            let t = wty(&self.types[decl.value.id as usize]);
            let c=wgsl_name(c);let _ = writeln!(self.out, "const {c}: {t} = {v};");
        }
        for f in fns {
            self.func(f, &wgsl_name(&f.name.name));
        }
    }
}

/// The lobe sequence of a kind's material: every `return` must build the same one (it fixes the `Bsdf` struct).
fn material_layout<'k>(p: &Prog, k: &'k KindDecl) -> Res<Vec<(&'k str, &'k [Arg])>> {
    fn returns<'b>(b: &'b Block, out: &mut Vec<&'b Expr>) {
        for s in &b.stmts {
            match s {
                Stmt::Return(Some(e), _) => out.push(e),
                Stmt::If { then, otherwise, .. } => {
                    returns(then, out);
                    otherwise.iter().for_each(|o| returns(o, out));
                }
                Stmt::Each{body,..} | Stmt::For { body, .. } => returns(body, out),
                Stmt::Return(None, _) | Stmt::Let { .. } | Stmt::Assign { .. } | Stmt::Map { .. } | Stmt::Emit { .. } | Stmt::Expr(_) => {}
            }
        }
    }
    let m = k.query(QueryKind::Material).expect("checked material");
    let mut rs = vec![];
    returns(&m.body, &mut rs);
    let mut layout: Option<Vec<(&str, &[Arg])>> = None;
    for e in rs {
        let mut ls = vec![];
        let ok = lobes(e, &mut ls) && ls.iter().all(|(n, _)| p.fns.get(n).is_some_and(|f| f.attrs.iter().any(|a| a.name.name == "lobe")));
        let same = layout.as_ref().is_none_or(|l| l.iter().map(|x| x.0).eq(ls.iter().map(|x| x.0)));
        if !ok || !same {
            return Err(Diag::new(Code::E405, e.span, "a material returns one sum of lobe calls, the same lobes on every path", "return `ggx(...) + clearcoat(...)` directly; vary the lobes' weights, not which lobes"));
        }
        layout = Some(ls);
    }
    layout.ok_or_else(|| Diag::new(Code::E003, m.name.span, "the material returns nothing", "return a sum of lobes"))
}

/// The text of one kind's module. `light` is the library fn the shade pass calls (named in the schedule).
pub fn kind_module(p: &Prog, types: &[Ty], k: &KindDecl, light: &str, luts: &[LutRef], draw: Draw, environment:[bool;2]) -> Res<String> {
    let mut em = Emitter::new(p, types);
    em.luts = luts;em.environment=environment;
    em.header(abi::NONCE_PLACEHOLDER);
    let q = |which| k.query(which).is_some();
    let mut roots: Vec<&FnDecl> = k.queries.iter().map(|(_, f)| f).collect();
    roots.extend(k.fns.iter());
    let (lib, consts) = reachable(p, &roots, &[light]);
    // The row: the author's fields, then the compiler's hidden columns.
    let mut fields = vec![];
    let mut loads = vec![];
    let mut o = 0u32;
    for rf in &k.row {
        let t = em.resolve(&rf.ty);
        let w = |i: u32| format!("rows[o + {}u]", o + i);
        let load = match &t {
            Ty::F32 => w(0),
            Ty::U32 | Ty::Id(_) => format!("bitcast<u32>({})", w(0)),
            Ty::I32 => format!("bitcast<i32>({})", w(0)),
            Ty::Bool => format!("({} != 0.0)", w(0)),
            Ty::VecN(n) => format!("vec{n}<f32>({})", (0..u32::from(*n)).map(w).collect::<Vec<_>>().join(", ")),
            Ty::Geo(..) => format!("vec3<f32>({}, {}, {})", w(0), w(1), w(2)),
            Ty::Xform(..) => format!(
                "LoftXform(mat3x3<f32>({}), vec3<f32>({}, {}, {}))",
                (0..9).map(w).collect::<Vec<_>>().join(", "),
                w(9),
                w(10),
                w(11)
            ),
            other @ Ty::Void | other @ Ty::AbstractInt | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Frame(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Bsdf => unreachable!("checked row field type {other}"),
        };
        fields.push(format!("  r_{}: {},", rf.name.name, wty(&t)));
        loads.push(load);
        o += t.words().expect("checked storable");
    }
    let stride = o + abi::HIDDEN_WORDS;
    fields.push("  loft_lo: vec3<f32>,\n  loft_hi: vec3<f32>,\n  loft_lip: f32,".into());
    loads.push(format!("vec3<f32>(rows[o + {o}u], rows[o + {}u], rows[o + {}u])", o + 1, o + 2));
    loads.push(format!("vec3<f32>(rows[o + {}u], rows[o + {}u], rows[o + {}u])", o + 3, o + 4, o + 5));
    loads.push(format!("rows[o + {}u]", o + 6));
    let pose = match crate::pose_field(k) {
        Some(i) => format!("r.r_{}", k.row[i].name.name),
        None => "LoftXform(mat3x3<f32>(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0), vec3<f32>(0.0))".into(),
    };
    let _ = write!(
        em.out,
        "// kind {name}: one module for every instance; rows are data.\nstruct Row {{\n{fields}\n}}\n\
         struct KindParams {{ kind_id: u32, base: u32, n_rows: u32, pad: u32 }}\n\
         @group(1) @binding(0) var<storage, read> rows: array<f32>;\n\
         @group(1) @binding(1) var<storage, read> visible: array<u32>;\n\
         @group(1) @binding(2) var<uniform> kp: KindParams;\n\
         @group(1) @binding(3) var<storage, read> luts: array<f32>;\n\
         const STRIDE: u32 = {stride}u;\n\
         fn load_row(i: u32) -> Row {{\n  let o = i * STRIDE;\n  return Row(\n    {loads}\n  );\n}}\n\
         // The row's placement in World: the compiler applies it, so queries are written in the kind's own space.\n\
         fn loft_pose(r: Row) -> LoftXform {{ return {pose}; }}\n\n{LUTS}",
        name = k.name.name,
        fields = fields.join("\n"),
        loads = loads.join(",\n    "),
    );
    // The Bsdf struct: one field per bound lobe argument, in the material's lobe order.
    let layout = material_layout(p, k)?;
    let mut bfields = vec![];
    let mut evals = String::new();
    for (i, (name, args)) in layout.iter().enumerate() {
        let f = p.fns[name];
        let mut names = vec![];
        for (j, prm) in f.params.iter().take(args.len()).enumerate() {
            bfields.push(format!("  l{i}_{j}: {},", wty(&em.resolve(prm.ty.as_ref().expect("typed")))));
            names.push(format!("b.l{i}_{j}"));
        }
        let name=wgsl_name(name);let _ = write!(evals, "  let r{i} = {name}({}, n, v, l);\n  c = c * r{i}.w + r{i}.xyz;\n", names.join(", "));
    }
    let _ = write!(
        em.out,
        "// The material's lobes, bottom first; `l = 0` asks for emitted radiance.\nstruct Bsdf {{\n{}\n}}\n\
         fn eval_bsdf(b: Bsdf, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec3<f32> {{\n  var c = vec3<f32>(0.0);\n{evals}  return c;\n}}\n\n",
        bfields.join("\n")
    );
    em.library(&lib, &consts);
    for f in &k.fns {
        em.func(f, &wgsl_name(&f.name.name));
    }
    for (which, f) in &k.queries {
        let n = match which {
            QueryKind::Visible => "q_visible",
            QueryKind::Trace => "q_trace",
            QueryKind::Material => "q_material",
            QueryKind::Surface => "q_surface",
        };
        em.func(f, n);
    }
    // Draw-form glue: the vis entry points, how shade rebuilds the hit, and the twin's geometry.
    let (vis, geometry, twin_geo) = match draw {
        Draw::Surface { nu, nv } => (
            format!(
                r"struct VsOut {{ @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) @interpolate(flat) row: u32 }}
struct FsOut {{ @location(0) vis: vec2<u32> }}

// The grid: two triangles per cell of {nu} x {nv}; corner bits (0,1,1,0,1,0) in u and (0,0,1,0,1,1) in v.
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {{
  let r = visible[kp.base + ii];
  let row = load_row(r);
  let cell = vi / 6u;
  let c = vi % 6u;
  let u = f32(cell % {nu}u + ((0x16u >> c) & 1u)) / {nu}.0;
  let v = f32(cell / {nu}u + ((0x34u >> c) & 1u)) / {nv}.0;
  let wp = loft_xf_pt(loft_pose(row), q_surface(row, u, v));
  return VsOut(frame.view_proj * vec4<f32>(wp, 1.0), vec2<f32>(u, v), r);
}}

// The record carries (u, v); shading rebuilds the point and the AD normal from them.
@fragment fn fs(v: VsOut) -> FsOut {{
  if frame.cold.x == LOFT_NONCE {{ discard; }}
  // Packed by hand: Metal's pack/unpack2x16unorm round trip through naga garbled u on the M4 (19 Nov 2026).
  let q = vec2<u32>(round(clamp(v.uv, vec2<f32>(0.0), vec2<f32>(1.0)) * 65535.0));
  return FsOut(vec2<u32>((kp.kind_id << 24u) | v.row, q.x | (q.y << 16u)));
}}
"
            ),
            "  let uv = vec2<f32>(f32(v.y & 0xffffu), f32(v.y >> 16u)) / 65535.0;
  let pl = q_surface(row, uv.x, uv.y);
  let cr = cross(surface__jvp(row, uv.x, uv.y, 1.0, 0.0), surface__jvp(row, uv.x, uv.y, 0.0, 1.0));
  let nl = cr * inverseSqrt(max(dot(cr, cr), 1e-30));
  let p = loft_xf_pt(pose, pl);
  var n = normalize(loft_xf_vec(pose, nl));
  // Two-sided: the inside of the aperture faces the camera too.
  if dot(n, rd) > 0.0 { n = -n; }
  let ao = 1.0;
  let b = q_material(row, pl, nl, uv);
"
            .to_string(),
            "  let pl = q_surface(row, a.x, a.y);
  let cr = cross(surface__jvp(row, a.x, a.y, 1.0, 0.0), surface__jvp(row, a.x, a.y, 0.0, 1.0));
  let nl = normalize(cr);
  let nw = loft_xf_vec(pose, nl);
  let o0 = vec4<f32>(0.0);
  let o2 = vec4<f32>(loft_xf_pt(pose, pl), 0.0);
  let o3 = vec4<f32>(nw, 0.0);
  let bs = q_material(row, pl, nl, a.xy);
"
            .to_string(),
        ),
        Draw::Analytic | Draw::Traced => {
            let hit = match draw {
                Draw::Analytic => "  // Closed form: the author's `visible` query, in the kind's space (a rigid pose keeps t).\n  let t = q_visible(row, lro, lrd);\n  if t < 0.0 { discard; }\n".to_string(),
                Draw::Traced | Draw::Surface { .. } => "  // Sphere trace with the row's derived Lipschitz bound (hidden column `loft_lip`).\n  \
                     var t = max(span.x, 0.0);\n  let step = 1.0 / row.loft_lip;\n  var hit = false;\n  \
                     for (var i = 0u; i < 192u; i++) {\n    let d = q_trace(row, lro + lrd * t);\n    \
                     if d < 1e-4 + 2e-4 * t { hit = true; break; }\n    t += d * step;\n    if t > span.y { break; }\n  }\n  \
                     if !hit { discard; }\n"
                    .to_string(),
            };
            let visible_twin = if q(QueryKind::Visible) { "q_visible(row, pl, dl)" } else { "0.0" };
            (
                format!(
                    r"const LOFT_CUBE: array<u32, 36> = array<u32, 36>(1u, 3u, 7u, 1u, 7u, 5u, 0u, 6u, 2u, 0u, 4u, 6u, 2u, 6u, 7u, 2u, 7u, 3u,
  0u, 1u, 5u, 0u, 5u, 4u, 4u, 5u, 7u, 4u, 7u, 6u, 0u, 2u, 3u, 0u, 3u, 1u);
struct VsOut {{ @builtin(position) pos: vec4<f32>, @location(0) wp: vec3<f32>, @location(1) @interpolate(flat) row: u32 }}
struct FsOut {{ @location(0) vis: vec2<u32>, @builtin(frag_depth) depth: f32 }}

// The proxy: the row's derived AABB, back faces (the camera may be inside it).
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {{
  let r = visible[kp.base + ii];
  let row = load_row(r);
  let c = LOFT_CUBE[vi];
  let corner = vec3<f32>(f32(c & 1u), f32((c >> 1u) & 1u), f32((c >> 2u) & 1u));
  let wp = mix(row.loft_lo, row.loft_hi, corner);
  return VsOut(frame.view_proj * vec4<f32>(wp, 1.0), wp, r);
}}

const LOFT_TETRA: array<vec3<f32>, 4> = array<vec3<f32>, 4>(vec3<f32>(1.0, -1.0, -1.0), vec3<f32>(-1.0, -1.0, 1.0),
  vec3<f32>(-1.0, 1.0, -1.0), vec3<f32>(1.0, 1.0, 1.0));

fn loft_box(ro: vec3<f32>, rd: vec3<f32>, lo: vec3<f32>, hi: vec3<f32>) -> vec2<f32> {{
  let inv = 1.0 / rd;
  let a = (lo - ro) * inv;
  let b = (hi - ro) * inv;
  let n = min(a, b);
  let f = max(a, b);
  return vec2<f32>(max(max(n.x, n.y), n.z), min(min(f.x, f.y), f.z));
}}

@fragment fn fs(v: VsOut) -> FsOut {{
  if frame.cold.x == LOFT_NONCE {{ discard; }}
  let row = load_row(v.row);
  var ro = frame.eye.xyz;
  var rd = normalize(v.wp - ro);
  // The sun's view is orthographic: every ray runs along `eye.xyz`, starting outside the row's bound.
  if frame.cold.y == 1u {{
    rd = frame.eye.xyz;
    ro = v.wp - rd * (2.0 * length(row.loft_hi - row.loft_lo) + 1.0);
  }}
  let span = loft_box(ro, rd, row.loft_lo, row.loft_hi);
  let xf = loft_xf_inv(loft_pose(row));
  let lro = loft_xf_pt(xf, ro);
  let lrd = loft_xf_vec(xf, rd);
{hit}  let clip = frame.view_proj * vec4<f32>(ro + rd * t, 1.0);
  return FsOut(vec2<u32>((kp.kind_id << 24u) | v.row, bitcast<u32>(t)), clip.z / clip.w);
}}
"
                ),
                "  let t = bitcast<f32>(v.y);
  let p = frame.eye.xyz + rd * t;
  let pl = loft_xf_pt(xf, p);
  // Normal from the field (tetrahedron differences, i < 4), then occlusion along it (i >= 4), in the kind's space.
  // One loop, one call site: Metal inlines every call site of the field, and compile time follows (phase1.md).
  let e = 5e-4 * max(1.0, 0.25 * t);
  var g = vec3<f32>(0.0);
  var nl = vec3<f32>(0.0, 1.0, 0.0);
  var occ = 0.0;
  var w = 1.0;
  for (var i = 0u; i < 9u; i++) {
    var q = pl + LOFT_TETRA[min(i, 3u)] * e;
    var h = 0.0;
    if i >= 4u {
      if i == 4u { nl = normalize(g); }
      h = 0.02 + 0.06 * f32(i - 4u);
      q = pl + nl * h;
    }
    let d = q_trace(row, q);
    if i < 4u { g += LOFT_TETRA[i] * d; } else { occ += (h - d) * w; w *= 0.8; }
  }
  let ao = clamp(1.0 - 2.0 * occ, 0.0, 1.0);
  let n = normalize(loft_xf_vec(pose, nl));
  let b = q_material(row, pl, nl);
"
                .to_string(),
                format!(
                    "  let pl = loft_xf_pt(xf, a.xyz);
  let dl = normalize(loft_xf_vec(xf, d.xyz));
  let o0 = vec4<f32>(q_trace(row, pl), {visible_twin}, 0.0, 0.0);
  let o2 = vec4<f32>(0.0);
  let o3 = vec4<f32>(0.0);
  let nl = dl;
  let nw = normalize(d.xyz);
  let bs = q_material(row, pl, nl);
"
                ),
            )
        }
    };
    let tl = crate::unit(crate::TWIN_LIGHT);
    let _ = write!(
        em.out,
        r"// --- generated entry points ---
{vis}
@group(2) @binding(0) var vis_tex: texture_2d<u32>;
@group(2) @binding(1) var hdr: texture_storage_2d<rgba16float, write>;
@group(2) @binding(2) var<storage, read> tiles: array<u32>;
@group(2) @binding(3) var shadow_tex: texture_depth_2d;
@group(2) @binding(4) var shadow_smp: sampler_comparison;

// The sun's shadow map, 3x3 bilinear comparisons; the point moves 1.5 texels along its normal first.
fn loft_shadow(p: vec3<f32>, n: vec3<f32>) -> f32 {{
  if frame.cold.z == 0u {{ return 1.0; }}
  let q = frame.light_vp * vec4<f32>(p + n * (1.5 * frame.sun.w), 1.0);
  let uv = vec2<f32>(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5);
  if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || q.z > 1.0 || q.z < 0.0 {{ return 1.0; }}
  let texel = 1.0 / f32(textureDimensions(shadow_tex).x);
  var s = 0.0;
  for (var i = -1; i <= 1; i++) {{
    for (var j = -1; j <= 1; j++) {{
      s += textureSampleCompareLevel(shadow_tex, shadow_smp, uv + vec2<f32>(f32(i), f32(j)) * texel, q.z - 0.0005);
    }}
  }}
  return s / 9.0;
}}

// One workgroup per 8x8 tile that classify listed for this kind.
@compute @workgroup_size(8, 8) fn shade(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {{
  if frame.cold.x == LOFT_NONCE {{ return; }}
  let tile = tiles[kp.kind_id * frame.size.w + wg.x];
  let px = vec2<u32>(tile & 0xffffu, tile >> 16u) * 8u + lid.xy;
  if px.x >= frame.size.x || px.y >= frame.size.y {{ return; }}
  let v = textureLoad(vis_tex, px, 0).xy;
  if v.x == LOFT_EMPTY || (v.x >> 24u) != kp.kind_id {{ return; }}
  let row = load_row(v.x & 0xffffffu);
  let rd = loft_ray(vec2<f32>(px) + 0.5);
  let pose = loft_pose(row);
  let xf = loft_xf_inv(pose);
{geometry}  let sun = normalize(frame.sun.xyz);
  textureStore(hdr, px, vec4<f32>({light}(b, n, -rd, sun, loft_shadow(p, n), ao), 1.0));
}}

@group(3) @binding(0) var<storage, read> twin_in: array<vec4<f32>>;
@group(3) @binding(1) var<storage, read_write> twin_out: array<vec4<f32>>;

// The twin gate: the frame's queries at CPU-chosen samples; the material is lit by one fixed light plus emission.
@compute @workgroup_size(64) fn twin(@builtin(global_invocation_id) g: vec3<u32>) {{
  let i = g.x;
  if i >= arrayLength(&twin_out) / 4u {{ return; }}
  let a = twin_in[2u * i];
  let d = twin_in[2u * i + 1u];
  let row = load_row(bitcast<u32>(a.w));
  let pose = loft_pose(row);
  let xf = loft_xf_inv(pose);
{twin_geo}  // Lit 60 degrees off the normal, away from any specular peak (where a 1e-7 change moves D by 1e-3).
  let lt = nw * 0.5 + normalize(cross(nw, vec3<f32>({l0}, {l1}, {l2}))) * 0.8660254;
  let m = eval_bsdf(bs, nw, nw, lt) + eval_bsdf(bs, nw, nw, vec3<f32>(0.0));
  twin_out[4u * i] = o0;
  twin_out[4u * i + 1u] = vec4<f32>(m, 0.0);
  twin_out[4u * i + 2u] = o2;
  twin_out[4u * i + 3u] = o3;
}}
",
        light = wgsl_name(light),
        l0 = f32_lit(f64::from(tl[0])),
        l1 = f32_lit(f64::from(tl[1])),
        l2 = f32_lit(f64::from(tl[2])),
    );
    Ok(em.out)
}

/// Scan tables: frame `i` of a scan at `off`, clamped to its ends; the uniform cubic B-spline (C2 in t) and its
/// t-derivative scaled by `dt`, the same weights the CPU evaluator uses.
const LUTS: &str = r"fn loft_lut(i: i32, off: u32, n: u32) -> LoftFrame {
  let o = (off + u32(clamp(i, 0, i32(n) - 1))) * 12u;
  return LoftFrame(vec3<f32>(luts[o], luts[o + 1u], luts[o + 2u]), vec3<f32>(luts[o + 3u], luts[o + 4u], luts[o + 5u]),
    vec3<f32>(luts[o + 6u], luts[o + 7u], luts[o + 8u]), vec3<f32>(luts[o + 9u], luts[o + 10u], luts[o + 11u]));
}
fn loft_blend(i: i32, off: u32, n: u32, w: vec4<f32>) -> LoftFrame {
  let a = loft_lut(i - 1, off, n);
  let b = loft_lut(i, off, n);
  let c = loft_lut(i + 1, off, n);
  let d = loft_lut(i + 2, off, n);
  return LoftFrame(a.origin * w.x + b.origin * w.y + c.origin * w.z + d.origin * w.w,
    a.tangent * w.x + b.tangent * w.y + c.tangent * w.z + d.tangent * w.w,
    a.normal * w.x + b.normal * w.y + c.normal * w.z + d.normal * w.w,
    a.binormal * w.x + b.binormal * w.y + c.binormal * w.z + d.binormal * w.w);
}
fn loft_sample(off: u32, n: u32, lo: f32, hi: f32, t: f32) -> LoftFrame {
  let s = (t - lo) / (hi - lo) * f32(n - 1u);
  let i = clamp(floor(s), 0.0, f32(n - 1u));
  let f = s - i;
  let g = 1.0 - f;
  return loft_blend(i32(i), off, n, vec4<f32>(g * g * g / 6.0, (3.0 * f * f * f - 6.0 * f * f + 4.0) / 6.0,
    (-3.0 * f * f * f + 3.0 * f * f + 3.0 * f + 1.0) / 6.0, f * f * f / 6.0));
}
fn loft_dsample(off: u32, n: u32, lo: f32, hi: f32, t: f32, dt: f32) -> LoftFrame {
  let s = (t - lo) / (hi - lo) * f32(n - 1u);
  let i = clamp(floor(s), 0.0, f32(n - 1u));
  let f = s - i;
  let g = 1.0 - f;
  let k = f32(n - 1u) / (hi - lo) * dt;
  return loft_blend(i32(i), off, n, vec4<f32>(-g * g / 2.0, (3.0 * f * f - 4.0 * f) / 2.0, (-3.0 * f * f + 2.0 * f + 1.0) / 2.0, f * f / 2.0) * k);
}

";

pub fn cull_module() -> String {
    let mut s = PRELUDE.replace("{{NONCE}}", abi::NONCE_PLACEHOLDER);
    s += r"
struct Inst { lo: vec3<f32>, kind: u32, hi: vec3<f32>, row: u32 }
@group(1) @binding(0) var<storage, read> insts: array<Inst>;
@group(1) @binding(1) var<storage, read_write> draw_args: array<atomic<u32>>;
@group(1) @binding(2) var<storage, read_write> visible: array<u32>;
@group(1) @binding(3) var<storage, read> bases: array<u32>;

// Every row of every kind: frustum-test its derived AABB, append visible rows, count one draw per kind.
@compute @workgroup_size(64) fn cull(@builtin(global_invocation_id) g: vec3<u32>) {
  if frame.cold.x == LOFT_NONCE { return; }
  let i = g.x;
  if i >= arrayLength(&insts) { return; }
  let b = insts[i];
  var outside = 0x3fu;
  for (var c = 0u; c < 8u; c++) {
    let corner = vec3<f32>(f32(c & 1u), f32((c >> 1u) & 1u), f32((c >> 2u) & 1u));
    let q = frame.view_proj * vec4<f32>(mix(b.lo, b.hi, corner), 1.0);
    var bits = 0u;
    if q.x < -q.w { bits |= 1u; }
    if q.x > q.w { bits |= 2u; }
    if q.y < -q.w { bits |= 4u; }
    if q.y > q.w { bits |= 8u; }
    if q.z < 0.0 { bits |= 16u; }
    if q.z > q.w { bits |= 32u; }
    outside &= bits;
  }
  if outside != 0u { return; }
  let slot = atomicAdd(&draw_args[b.kind * 4u + 1u], 1u);
  visible[bases[b.kind] + slot] = b.row;
}
";
    s
}

pub fn classify_module() -> String {
    let mut s = PRELUDE.replace("{{NONCE}}", abi::NONCE_PLACEHOLDER);
    s += r"
@group(1) @binding(0) var vis_tex: texture_2d<u32>;
@group(1) @binding(1) var<storage, read_write> shade_args: array<atomic<u32>>;
@group(1) @binding(2) var<storage, read_write> tiles: array<u32>;
var<workgroup> mask: atomic<u32>;

// One workgroup per 8x8 tile: which kinds cover it; append the tile to each kind's list.
@compute @workgroup_size(8, 8) fn classify(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_index) li: u32,
    @builtin(global_invocation_id) g: vec3<u32>) {
  if frame.cold.x == LOFT_NONCE { return; }
  if li == 0u { atomicStore(&mask, 0u); }
  workgroupBarrier();
  if g.x < frame.size.x && g.y < frame.size.y {
    let v = textureLoad(vis_tex, g.xy, 0).x;
    if v != LOFT_EMPTY { atomicOr(&mask, 1u << (v >> 24u)); }
  }
  workgroupBarrier();
  if li == 0u {
    var m = atomicLoad(&mask);
    loop {
      if m == 0u { break; }
      let k = firstTrailingBit(m);
      let slot = atomicAdd(&shade_args[k * 3u], 1u);
      tiles[k * frame.size.w + slot] = wg.x | (wg.y << 16u);
      m &= m - 1u;
    }
  }
}
";
    s
}

/// `sky` is the library fn named by the schedule's present pass.
pub fn present_module(p: &Prog, types: &[Ty], sky: &str, environment:[bool;2]) -> String {
    let mut em = Emitter::new(p, types);
    em.header(abi::NONCE_PLACEHOLDER);
    em.environment=environment;let (lib, consts) = reachable(p, &[], &[sky]);
    em.library(&lib, &consts);
    let sky=wgsl_name(sky);
    em.out += &format!(
        r"@group(1) @binding(0) var hdr_tex: texture_2d<f32>;
@group(1) @binding(1) var vis_tex: texture_2d<u32>;

@vertex fn vs_present(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {{
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}}

// ACES filmic fit (Narkowicz 2015); the target is sRGB, so the hardware encodes.
fn loft_aces(x: vec3<f32>) -> vec3<f32> {{
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}}

@fragment fn fs_present(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {{
  if frame.cold.x == LOFT_NONCE {{ discard; }}
  let px = vec2<u32>(pos.xy);
  var c = textureLoad(hdr_tex, px, 0).rgb;
  if textureLoad(vis_tex, px, 0).x == LOFT_EMPTY {{ c = {sky}(loft_ray(pos.xy), normalize(frame.sun.xyz)); }}
  return vec4<f32>(loft_aces(c), 1.0);
}}
"
    );
    em.out
}
