//! WGSL emit (plan §1 layer 2): one module per kind, never per instance, and one per schedule-authored frame pass
//! (the stock cull, classify and present). A kind's text depends only on its declaration and the library fns it reaches; its id, row base and row
//! count are data in `KindParams`. The author's structure is kept: their fns stay fns, their loops stay loops.

use loft_check::{Prog, FINITE_DIFF_STEP, TWIN_LIGHT, finite_diff_parameter, pose_field, unit};
pub use loft_check::references::{References, fn_symbol, proof_dependencies, reachable};
use loft_builtins::{MotionClass, WgslLowering, lookup};
#[path="aov.rs"]
mod aov;
pub use aov::Aov;
pub mod paths;
#[path="material_shape.rs"]
mod material_shape;
mod surface;
mod lighting;
pub mod shade_tiles;
mod column;
use material_shape::{Shape, fn_shape, is_lobe, shape_of};
use loft_parse::ast::*;
use loft_check::program::{abi, Draw};
use loft_ir::{Code, Diag, Res};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

const PRELUDE: &str = include_str!("../../../stdlib/core/prelude.wgsl");

// Substitute only markers in the template. Inserted WGSL may itself carry the runtime nonce.
fn render_template(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some((before, marker)) = rest.split_once("{{") {
        let (key, after) = marker.split_once("}}").expect("closed WGSL template marker");
        out.push_str(before);
        if let Some((_, value)) = values.iter().find(|(name, _)| *name == key) {
            out.push_str(value);
        } else {
            debug_assert_eq!(key, "NONCE", "unreplaced WGSL template marker");
            out.push_str("{{");
            out.push_str(key);
            out.push_str("}}");
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

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
        Ty::Array(t, n) => format!("array<{}, {n}>", wty(t)),
        Ty::Struct(k) if k == "World" => "u32".into(),
        Ty::Struct(k) => format!("loft_type_{k}"),
        Ty::Verdict => "CheckValue".into(),
        Ty::Table(_) => unreachable!("motion tables are lowered in a law"),
        Ty::Map(..) => unreachable!("maps are not values"),
    }
}

// Typed Loft locals and generated WGSL symbols use separate namespaces.
fn member_name(n: &str) -> String {
    if n == "emit" { "r_emit".into() } else { n.into() }
}

pub fn wgsl_name(n: &str) -> String { format!("loft_local_{n}") }
fn map_symbol(n: &str) -> String { format!("loft_map_{}_{}", n.len(), n) }

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
    pub out: String,
    maps: Vec<(String, Vec<MapStage>)>,
    luts: &'p [LutRef],
    pub motion:bool,
    pub motion_tier: Determinism,
    pub locals: Vec<String>,
    temp: u32,
    prepare_light: bool,
    in_prepared_light: bool,
    baked_column: Option<u32>,
    sampled_bakes: BTreeSet<String>,
    kernel_atomics: BTreeMap<(String, String), AtomicScalar>,
    kernel_sampling: bool,
    /// Kind parameters as WGSL literals, by (kind, field): the one variant this scene's rows use.
    pub fixed: &'p BTreeMap<(String, String), String>,
    /// Tables this module reads, by kind: the WGSL loader `K[i]` and `legs[i]` lower to.
    pub tables: BTreeMap<String, String>,
    /// Kinds whose rows have their own struct `Row_K` here (every kind but the module's own `Row`).
    pub foreign_rows: BTreeSet<String>,
    /// The kind's material shape, stored in the struct named `Bsdf`.
    material: Option<Shape>,
    /// Bsdf lets and params in scope, innermost last; `None` for a local of another type that shadows one.
    bsdf_locals: Vec<(String, Option<Shape>)>,
    /// Other Bsdf structs this module uses, by name: let-bound stacks and helpers' return values.
    bsdf_structs: BTreeMap<String, String>,
}

static NO_FIXED: BTreeMap<(String, String), String> = BTreeMap::new();

impl<'p, 'a> Emitter<'p, 'a> {
    pub fn new(p: &'p Prog<'a>, types: &'p [Ty]) -> Self {
        Emitter { p, types, out: String::new(), maps: vec![], luts: &[],motion:false,motion_tier:Determinism::Det,locals:vec![],temp:0,
            prepare_light: false, in_prepared_light: false, baked_column: None, sampled_bakes: BTreeSet::new(), kernel_atomics: BTreeMap::new(), kernel_sampling: false, fixed: &NO_FIXED,
            tables: BTreeMap::new(), foreign_rows: BTreeSet::new(), material: None, bsdf_locals: vec![], bsdf_structs: BTreeMap::new() }
    }

    fn ty_of(&self, e: &Expr) -> &Ty {
        &self.types[e.id as usize]
    }

    /// `wty`, with a row of another kind read in this module lowered to its own struct.
    fn wt(&self, t: &Ty) -> String {
        match t {
            Ty::Row(k) if self.foreign_rows.contains(k) => format!("Row_{k}"),
            _ => wty(t),
        }
    }

    /// A table value (`[K]`) has no WGSL form: its rows are a module binding, so it leaves signatures and calls.
    fn is_table(&self, e: &Expr) -> bool {
        matches!(self.ty_of(e), Ty::Table(_))
    }

    pub fn resolve(&self, t: &TyExpr) -> Ty {
        loft_check::check::Ck::new(self.p, &mut vec![]).resolve(t, true).expect("checked signature")
    }

    pub fn func(&mut self, f: &FnDecl, name: &str) {
        let mut params: Vec<String> = f
            .params
            .iter()
            .filter(|prm| !matches!(&prm.ty, Some(TyExpr::Table(_))))
            .map(|prm| match &prm.ty {
                Some(TyExpr::Name(n)) if self.p.kinds.contains_key(n.name.as_str())=>
                    format!("{}: {}",wgsl_name(&prm.name.name),self.wt(&Ty::Row(n.name.clone()))),
                Some(t) => {
                    let name = if f.raw.is_some() { prm.name.name.clone() } else { wgsl_name(&prm.name.name) };
                    format!("{name}: {}", self.wt(&self.resolve(t)))
                }
                None => format!("{}: Row", wgsl_name(&prm.name.name)),
            })
            .collect();
        if self.baked_column.is_some() {
            params.push("loft_column_value: f32".into());
        }
        let ret = f.ret.as_ref().map(|t| match self.resolve(t) {
            Ty::Bsdf => format!(" -> {}", self.fn_struct(f)),
            t => format!(" -> {}", self.wt(&t)),
        }).unwrap_or_default();
        let _ = writeln!(self.out, "fn {name}({}){ret} {{", params.join(", "));
        if let Some(raw)=&f.raw{
            self.out.push_str(&crate::provenance::raw(f.name.span,&raw.body[1..raw.body.len()-1]));self.out.push_str("\n}\n");return;
        }
        self.maps.clear();
        self.locals = f.params.iter().map(|p| p.name.name.clone()).collect();
        // A Bsdf parameter (a light's or a style's `b`) is the kind's material.
        self.bsdf_locals = f.params.iter().map(|prm| {
            let bsdf = prm.ty.as_ref().is_some_and(|t| matches!(t, TyExpr::Name(n) if n.name == "Bsdf"));
            (prm.name.name.clone(), if bsdf { self.material.clone() } else { None })
        }).collect();
        self.in_prepared_light = self.prepare_light && f.name.name == "sun_sky"
            && self.locals.get(..3).is_some_and(|keys| keys == ["b", "n", "v"]);
        if self.in_prepared_light {
            self.line(1, "let loft_lighting = loft_prepare_bsdf(loft_local_b, loft_local_n, loft_local_v);");
        }
        self.block_body(&f.body, 1);
        self.in_prepared_light = false;
        self.out += "}\n\n";
    }

    pub fn block_body(&mut self, b: &Block, depth: usize) {
        let mark = self.maps.len();
        let (local_mark, bsdf_mark) = (self.locals.len(), self.bsdf_locals.len());
        for s in &b.stmts {
            self.stmt(s, depth);
        }
        self.maps.truncate(mark);
        self.locals.truncate(local_mark);
        self.bsdf_locals.truncate(bsdf_mark);
    }

    fn line(&mut self, depth: usize, s: &str) {
        let _ = writeln!(self.out, "{}{s}", "  ".repeat(depth));
    }

    fn stmt(&mut self, s: &Stmt, d: usize) {
        match s {
            Stmt::Let { mutable, name, value, .. } => {
                let v = self.expr(value);
                let shape = (*self.ty_of(value) == Ty::Bsdf).then(|| self.shape(value));
                let t = match &shape { Some(s) => self.bsdf_struct(s), None => self.wt(self.ty_of(value)) };
                let kw = if *mutable { "var" } else { "let" };
                self.line(d, &format!("{kw} {}: {t} = {v};", wgsl_name(&name.name)));
                self.locals.push(name.name.clone());
                self.bsdf_locals.push((name.name.clone(), shape));
            }
            Stmt::Assign { target, op, value } => {
                let (t, v) = (self.expr(target), self.expr(value));
                let op = op.map_or("=".to_string(), |o| format!("{}=", o.wgsl()));
                self.line(d, &format!("{t} {op} {v};"));
            }
            Stmt::Map { name, head, stages } => {
                let n = &map_symbol(&name.name);
                let (point, mut all): (String, Vec<(&str, Vec<String>)>) = match &head.kind {
                    ExprKind::Call { callee, args } if loft_check::check::MAP_OPS.contains(&callee.name.as_str()) => {
                        (self.expr(&args[0].value), vec![(callee.name.as_str(), vec![])])
                    }
                    ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::If { .. } | ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Call { .. } | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => (self.expr(head), vec![]),
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
                self.line(d, &format!("let {}: vec3<f32> = {n}_s{};", wgsl_name(&name.name), all.len()));
                self.locals.push(name.name.clone());
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
                    Ty::Void | Ty::Bool | Ty::I32 | Ty::F32 | Ty::AbstractInt | Ty::VecN(_) | Ty::Geo(..) | Ty::Xform(..) | Ty::Map(..) | Ty::Row(_) | Ty::Frame(_) | Ty::Table(_) | Ty::Verdict | Ty::Struct(_) | Ty::Array(..) | Ty::Id(_) | Ty::Bsdf => "i32",
                });
                let (a, b) = (self.expr(lo), self.expr(hi));
                let v = wgsl_name(&var.name);
                self.line(d, &format!("for (var {v}: {t} = {a}; {v} < {b}; {v}++) {{"));
                self.locals.push(var.name.clone());
                self.block_body(body, d + 1);
                self.locals.pop();
                self.line(d, "}");
            }
            Stmt::Each{..} | Stmt::Emit { .. } | Stmt::ImportTable { .. } => unreachable!("checked: stage 0 only, never emitted"),
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

    /// The shape of a Bsdf expression in the current fn; the checker proved it static (material_layout).
    fn shape(&self, e: &Expr) -> Shape {
        let locals = &self.bsdf_locals;
        let local = |n: &str| locals.iter().rev().find(|(name, _)| name == n).and_then(|(_, s)| s.clone());
        shape_of(self.p, e, &local, &mut BTreeSet::new()).expect("checked: a Bsdf expression has a static layout")
    }

    /// The struct a Bsdf fn returns. A Bsdf fn outside every material chain was never shape-checked; it keeps
    /// the kind's struct, as before Bsdf values composed, and naga reports a mismatch.
    fn fn_struct(&mut self, f: &FnDecl) -> String {
        match fn_shape(self.p, f, &mut BTreeSet::new()) {
            Ok(shape) => self.bsdf_struct(&shape),
            Err(_) => "Bsdf".into(),
        }
    }

    /// The WGSL struct holding values of `shape`, declared once per module.
    fn bsdf_struct(&mut self, shape: &Shape) -> String {
        if self.material.as_ref().is_some_and(|m| m.key() == shape.key()) {
            return "Bsdf".into();
        }
        let key = shape.key();
        let hash = key.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3));
        let name = format!("LoftBsdf_{hash:016x}");
        if !self.bsdf_structs.contains_key(&name) {
            let mut fields = vec![];
            for (i, (lobe, arity)) in shape.lobes.iter().enumerate() {
                for (j, prm) in self.p.fns[lobe.as_str()].params.iter().take(*arity).enumerate() {
                    fields.push(format!("  l{i}_{j}: {},", wty(&self.resolve(prm.ty.as_ref().expect("typed")))));
                }
            }
            fields.extend((0..shape.weights).map(|k| format!("  mix{k}: f32,")));
            self.bsdf_structs.insert(name.clone(), format!("struct {name} {{\n{}\n}}\n", fields.join("\n")));
        }
        name
    }

    /// A Bsdf expression's shape, its lobe fields and its weight fields, in struct order. A let, a parameter
    /// or a helper's result contributes the fields of its own struct; a helper call is evaluated once.
    fn bsdf_fields(&mut self, e: &Expr) -> (Shape, Vec<String>, Vec<String>) {
        match &e.kind {
            ExprKind::Binary(BinOp::Add, a, b) => {
                let ((sa, mut la, mut wa), (sb, lb, wb)) = (self.bsdf_fields(a), self.bsdf_fields(b));
                la.extend(lb);
                wa.extend(wb);
                (Shape::join(sa, sb, false), la, wa)
            }
            ExprKind::Call { callee, args } if callee.name == "mix_bsdf" => {
                let [a, b, w] = args.as_slice() else { unreachable!("checked mix_bsdf arity") };
                let ((sa, mut la, mut wa), (sb, lb, wb)) = (self.bsdf_fields(&a.value), self.bsdf_fields(&b.value));
                la.extend(lb);
                wa.extend(wb);
                wa.push(self.expr(&w.value));
                (Shape::join(sa, sb, true), la, wa)
            }
            ExprKind::Call { callee, args } if is_lobe(self.p, &callee.name) => {
                (Shape::lobe(&callee.name, args.len()), args.iter().map(|a| self.expr(&a.value)).collect(), vec![])
            }
            _ => {
                let shape = self.shape(e);
                let mut value = self.expr(e);
                if !matches!(e.kind, ExprKind::Name(_)) {
                    let name = format!("loft_tmp_{}", self.temp);
                    self.temp += 1;
                    let ty = self.bsdf_struct(&shape);
                    self.line(1, &format!("let {name}: {ty} = {value};"));
                    value = name;
                }
                let lobes = shape.lobes.iter().enumerate()
                    .flat_map(|(i, (_, arity))| (0..*arity).map(move |j| (i, j)))
                    .map(|(i, j)| format!("{value}.l{i}_{j}")).collect();
                let weights = (0..shape.weights).map(|k| format!("{value}.mix{k}")).collect();
                (shape, lobes, weights)
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
        if self.baked_column == Some(e.id) {
            return "loft_column_value".into();
        }
        // A built Bsdf value is the struct of its lobes' bound arguments, then its mix weights (material_shape).
        if *self.ty_of(e) == Ty::Bsdf && (matches!(&e.kind, ExprKind::Binary(BinOp::Add, ..))
            || matches!(&e.kind, ExprKind::Call { callee, .. } if callee.name == "mix_bsdf" || is_lobe(self.p, &callee.name))) {
            let (shape, mut fields, weights) = self.bsdf_fields(e);
            fields.extend(weights);
            return format!("{}({})", self.bsdf_struct(&shape), fields.join(", "));
        }
        match &e.kind {
            ExprKind::Unproved { body, .. } => {
                if body.body.stmts.is_empty() {
                    return self.expr(&body.value);
                }
                let name = format!("loft_tmp_{}", self.temp);
                self.temp += 1;
                let ty = self.wt(self.ty_of(e));
                self.line(1, &format!("var {name}: {ty};"));
                self.line(1, "{");
                let (locals, maps) = (self.locals.len(), self.maps.len());
                for stmt in &body.body.stmts { self.stmt(stmt, 2); }
                let value = self.expr(&body.value);
                self.line(2, &format!("{name} = {value};"));
                self.locals.truncate(locals);
                self.maps.truncate(maps);
                self.line(1, "}");
                name
            }
            ExprKind::FiniteDiff { function, parameter, args } => {
                let f = self.p.fns[function.name.as_str()];
                let param = finite_diff_parameter(f, parameter);
                let mut args = args.iter().map(|a| self.expr(&a.value)).collect::<Vec<_>>();
                let value = args[param].clone();
                let step = FINITE_DIFF_STEP;
                let symbol = fn_symbol(&function.name);
                args[param] = format!("({value} + {step})");
                let hi = args.join(", ");
                args[param] = format!("({value} - {step})");
                let lo = args.join(", ");
                format!("(({symbol}({hi}) - {symbol}({lo})) / {})", 2.0 * step)
            }
            ExprKind::If { condition, then, otherwise } => {
                let name = format!("loft_tmp_{}", self.temp);
                self.temp += 1;
                let condition = self.expr(condition);
                let ty = self.wt(self.ty_of(e));
                self.line(1, &format!("var {name}: {ty};"));
                self.line(1, &format!("if {condition} {{"));
                for (i, arm) in [then, otherwise].into_iter().enumerate() {
                    let (locals, maps) = (self.locals.len(), self.maps.len());
                    if i != 0 { self.line(1, "} else {"); }
                    for stmt in &arm.body.stmts { self.stmt(stmt, 2); }
                    let value = self.expr(&arm.value);
                    self.line(2, &format!("{name} = {value};"));
                    self.locals.truncate(locals);
                    self.maps.truncate(maps);
                }
                self.line(1, "}");
                name
            }
            ExprKind::Num { value, suffix, text, .. } => match self.ty_of(e) {
                Ty::U32 => format!("{}u", *value as u32),
                Ty::I32 => format!("{}i", *value as i32),
                Ty::AbstractInt => text.clone(),
                Ty::Void | Ty::Bool | Ty::F32 | Ty::VecN(_) | Ty::Geo(..) | Ty::Xform(..) | Ty::Map(..) | Ty::Row(_) | Ty::Frame(_) | Ty::Table(_) | Ty::Verdict | Ty::Struct(_) | Ty::Array(..) | Ty::Id(_) | Ty::Bsdf => f32_lit(if *suffix == NumSuffix::Deg { value.to_radians() } else { *value }),
            },
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::String(_) => unreachable!("Stage 0 literal"),
            ExprKind::Name(n) if n=="time" && !self.locals.contains(n)=>if self.motion{"loft_tick_time"}else{"frame.eye.w"}.into(),
            ExprKind::Name(n) if !self.locals.contains(n) && self.p.consts.contains_key(n.as_str()) => {
                if let Some(i) = self.p.param_slot(n) {
                    format!("loft_params[{i}u].x")
                } else {
                    let suffix = if self.p.runtime_constant(n) { "()" } else { "" };
                    let name = format!("{}_{}{suffix}", if self.motion { "motion_const" } else { "loft_const" }, n);
                    // F9b: `const N = 36` is declared `i32`, but the checker types `N` as u32 in `for i in 0..N`
                    // (two abstract integers count in u32). WGSL converts neither way implicitly, so convert here.
                    let (declared, used) = (wty(self.ty_of(&self.p.consts[n.as_str()].value)), wty(self.ty_of(e)));
                    let scalar = |t: &str| matches!(t, "u32" | "i32" | "f32");
                    if declared != used && scalar(&declared) && scalar(&used) { format!("{used}({name})") } else { name }
                }
            }
            ExprKind::Name(n) if n == "SceneWorld" && !self.locals.contains(n) => "0u".into(),
            ExprKind::Name(n) if self.p.worlds.contains_key(n.as_str()) && !self.locals.contains(n) =>
                format!("{}u", 1 + self.p.worlds.keys().position(|k| *k == n).expect("checked world")),
            ExprKind::Name(n) => wgsl_name(n),
            ExprKind::Member(base, m) => {
                if let Some((table, _, field)) = atomic_target(e)
                    && let Some(ty) = self.kernel_atomics.get(&(table.to_owned(), field.to_owned())).cloned()
                {
                    let location = self.member(base, m);
                    let load = format!("atomicLoad(&{location})");
                    return match ty {
                        AtomicScalar::U32 | AtomicScalar::I32 | AtomicScalar::Float(FloatAtomics::Native) => load,
                        AtomicScalar::Float(FloatAtomics::Fixed(_)) => format!("loft_fixed_decode({load})"),
                    };
                }
                if let ExprKind::Name(n) = &base.kind
                    && m.name == "cell"
                    && self.maps.iter().any(|(k, _)| k == &map_symbol(n))
                {
                    return format!("{}_cell",map_symbol(n));
                }
                self.member(base, m)
            }
            // Rows of a bound kind table; a kernel's `[T]` tables keep their array form.
            ExprKind::Index(a, i) if matches!(self.ty_of(a), Ty::Table(k) if self.tables.contains_key(k)) => {
                let Ty::Table(k) = self.ty_of(a) else { unreachable!("matched a table") };
                let loader = self.tables[k].clone();
                format!("{loader}(u32({}))", self.expr(i))
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
                        self.map_apply(&map_symbol(n), m.name == "to_parent", x, point)
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
                    t @ Ty::Void | t @ Ty::Bool | t @ Ty::U32 | t @ Ty::I32 | t @ Ty::F32 | t @ Ty::AbstractInt | t @ Ty::VecN(_) | t @ Ty::Geo(..) | t @ Ty::Map(..) | t @ Ty::Row(_) | t @ Ty::Frame(_) | t @ Ty::Table(_) | t @ Ty::Verdict | t @ Ty::Struct(_) | t @ Ty::Array(..) | t @ Ty::Id(_) | t @ Ty::Bsdf => format!("{}({})", wty(t), a.join(", ")),
                }
            }
            ExprKind::Call { callee, args } if matches!(callee.name.as_str(), "sample" | "sample_lod")
                && !self.p.fns.contains_key(callee.name.as_str()) => {
                let ExprKind::Name(name) = &args[0].value.kind else { unreachable!("checked texture name") };
                self.sampled_bakes.insert(name.clone());
                let coord = self.expr(&args[1].value);
                let lod = args.get(2).map_or_else(|| "0.0".into(), |a| self.expr(&a.value));
                crate::bake_tex::sample_expression(self.p.bakes[name.as_str()], &coord, &lod, self.kernel_sampling)
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
                    other @ Ty::Void | other @ Ty::Bool | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Struct(_) | other @ Ty::Array(..) | other @ Ty::Id(_) | other @ Ty::Bsdf => unreachable!("checked zero of {other}"),
                }
            }
            ExprKind::Call { callee, args } if callee.name == "loft_vec" => self.expr(&args[0].value),
            ExprKind::Call { callee, args } if self.in_prepared_light && callee.name == "eval_bsdf"
                && args.len() == 4 && args.iter().zip(["b", "n", "v"]).all(|(arg, key)|
                    matches!(&arg.value.kind, ExprKind::Name(name) if name == key)
                    && self.locals.iter().filter(|name| name.as_str() == key).count() == 1) =>
            {
                let l = self.expr(&args[3].value);
                format!("loft_eval_prepared(loft_local_b, loft_local_n, loft_local_v, {l}, loft_lighting)")
            }
            ExprKind::Call { callee, args } => {
                if callee.name == "unknown" && !self.p.fns.contains_key("unknown") {
                    let ExprKind::Name(reason) = &args[0].value.kind else { unreachable!("checked reason") };
                    return format!("CheckValue(3u, {}u, 0.0)", loft_check::reason(reason).expect("checked reason") as u32);
                }
                if callee.name == "emit_event" {
                    let tag = match &args[0].value.kind {
                        ExprKind::Name(name) => format!("{}u", crate::events::event_tag(name)),
                        _ => self.expr(&args[0].value),
                    };
                    let args = args[1..].iter().map(|arg| self.expr(&arg.value)).collect::<Vec<_>>();
                    return format!("emit_event({tag}, {})", args.join(", "));
                }
                if callee.name == "atomic_add" {
                    let (table, index, field) = atomic_target(&args[0].value).expect("checked atomic destination");
                    let value = self.expr(&args[1].value);
                    if matches!(self.kernel_atomics.get(&(table.to_owned(), field.to_owned())), Some(AtomicScalar::Float(FloatAtomics::Fixed(_)))) {
                        let index = self.expr(index);
                        return format!("{}(u32({index}), {value})", float_atomic_symbol(table, field));
                    }
                    let ExprKind::Member(base, member) = &args[0].value.kind else { unreachable!("checked atomic destination") };
                    return format!("atomicAdd(&{}, {value})", self.member(base, member));
                }
                let kept: Vec<&Arg> = args.iter().filter(|x| !self.is_table(&x.value)).collect();
                let a: Vec<String> = kept.into_iter().map(|x| self.expr(&x.value)).collect();
                if callee.name=="place"{return format!("loft_xform_yaw({}, {})",a[0],a.get(1).map(String::as_str).unwrap_or("0.0"))}
                match callee.name.as_str(){
                    "workgroup_barrier" => return "workgroupBarrier()".into(),
                    "subgroup_add" => return format!("subgroupAdd({})", a[0]),
                    "global_id" => return format!("loft_gid[{}]", a[0]),
                    "sky_zenith"=>return "loft_params[1u].xyz".into(),
                    "sky_horizon"=>return "loft_params[2u].xyz".into(),
                    "has_sky"=>return "(loft_params[1u].w != 0.0)".into(),
                    "sun_color"=>return "loft_params[0u].xyz".into(),
                    _=>{}
                }
                if callee.name=="coords"{return a[0].clone()}
                if self.p.fns.contains_key(callee.name.as_str()) {return format!("{}{}({})",if self.motion{"motion_"}else{""},fn_symbol(&callee.name),a.join(", "));}
                if let Ok(Some(call)) = loft_check::world::WorldCall::resolve(&callee.name, &a, e.span) {
                    use loft_check::world::WorldQuery::*;
                    let world = call.world.as_deref().unwrap_or("0u");
                    match call.query {
                        Trace { ray } => return format!("loft_trace({world}, {ray})"),
                        Visible { a, b } => return format!("loft_visible({world}, {a}, {b})"),
                        Sky { direction } => {
                            let key = self.p.linked(callee.span, "sky_dome").expect("checked sky binding");
                            return format!("{}({direction})", fn_symbol(key));
                        }
                        Medium { point } => return format!("loft_medium({world}, {point})"),
                        Material { .. } | Light { .. } => {}
                    }
                }
                if callee.name == "inscatter_joint" {
                    let steps = if a.len() == 5 { a[3].as_str() } else { "24u" };
                    return format!("loft_authored_volume({}, {}, {}, {steps}, {})", a[0], a[1], a[2], a.last().unwrap());
                }
                let name = callee.name.as_str();
                if matches!(name, "held" | "missed") {
                    let status = if name == "held" { 1 } else { 2 };
                    return format!("CheckValue({status}u, 0u, {})", a[0]);
                }
                let symbol = match lookup(name) {
                    Some(spec) if self.motion && self.motion_tier.is_det()
                        && spec.motion_math == MotionClass::Fixed =>
                        return format!("det_{}({})", spec.name, a.join(", ")),
                    Some(spec) => match spec.wgsl {
                        WgslLowering::Direct(symbol) => symbol,
                        WgslLowering::Prefix(op) => return format!("({op}{})", a[0]),
                        WgslLowering::Infix(op) => return format!("({} {op} {})", a[0], a[1]),
                        // Generated BSDF evaluation retains its WGSL name.
                        WgslLowering::Custom => name,
                    },
                    None => name,
                };
                format!("{symbol}({})", a.join(", "))
            }
        }
    }

    fn member(&mut self, base: &Expr, name: &Ident) -> String {
        if let Ty::Row(kind) = self.ty_of(base)
            && let Some(value) = self.fixed.get(&(kind.clone(), name.name.clone())) {
            return value.clone();
        }
        let field = match self.ty_of(base) {
            Ty::Frame(_) => frame_field(&name.name).to_owned(),
            Ty::Row(_) => format!("r_{}", name.name),
            _ => member_name(&name.name),
        };
        format!("{}.{field}", self.expr(base))
    }

    pub fn header(&mut self, nonce: &str) {
        self.out += &render_template(PRELUDE, &[("NONCE", nonce)]);
        self.out += concat!(include_str!("../../../stdlib/core/xform_yaw.wgsl"), "\n");
    }

    fn sampled_textures(&mut self) {
        self.out.push_str(&crate::bake_tex::sampled_declarations(self.p, &self.sampled_bakes, self.kernel_sampling));
    }

    /// The row struct's fields and their loads from `buffer` (word `o` is the row's first), hidden columns included.
    fn row_parts(&self, k: &KindDecl, buffer: &str) -> (Vec<String>, Vec<String>, u32) {
        let mut fields = vec![];
        let mut loads = vec![];
        let mut o = 0u32;
        for rf in &k.row {
            let t = self.resolve(&rf.ty);
            let w = |i: u32| format!("{buffer}[o + {}u]", o + i);
            let fixed = self.fixed.get(&(k.name.name.clone(), rf.name.name.clone()));
            let load = if let Some(value) = fixed { value.clone() } else { match &t {
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
                other @ Ty::Void | other @ Ty::AbstractInt | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Frame(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Struct(_) | other @ Ty::Array(..) | other @ Ty::Bsdf => unreachable!("checked row field type {other}"),
            } };
            fields.push(format!("  r_{}: {},", rf.name.name, wty(&t)));
            loads.push(load);
            o += t.words().expect("checked storable");
        }
        let stride = o + abi::HIDDEN_WORDS;
        fields.push("  loft_lo: vec3<f32>,\n  loft_hi: vec3<f32>,\n  loft_lip: f32,".into());
        loads.push(format!("vec3<f32>({buffer}[o + {o}u], {buffer}[o + {}u], {buffer}[o + {}u])", o + 1, o + 2));
        loads.push(format!("vec3<f32>({buffer}[o + {}u], {buffer}[o + {}u], {buffer}[o + {}u])", o + 3, o + 4, o + 5));
        loads.push(format!("{buffer}[o + {}u]", o + 6));
        (fields, loads, stride)
    }

    /// A loader `fn {loader}(i: u32) -> {name}` for rows of `k` in the storage array `buffer` (declared by the caller);
    /// a `name` other than `Row` also gets its struct. The loads are the ones `row_header` writes.
    pub fn row_loader(&mut self, k: &KindDecl, name: &str, buffer: &str, loader: &str) {
        let (fields, loads, stride) = self.row_parts(k, buffer);
        if name != "Row" {
            let _ = write!(self.out, "struct {name} {{\n{}\n}}\n", fields.join("\n"));
        }
        let _ = write!(self.out, "fn {loader}(i: u32) -> {name} {{\n  let o = i * {stride}u;\n  return {name}(\n    {}\n  );\n}}\n",
            loads.join(",\n    "));
    }

    /// Bind another kind's rows read-only at (`group`, `binding`) and give them a loader, `K[i]` and `legs[i]` lower to it.
    pub fn foreign_table(&mut self, k: &KindDecl, group: u32, binding: u32) {
        let name = &k.name.name;
        let _ = writeln!(self.out, "@group({group}) @binding({binding}) var<storage, read> loft_rows_{name}: array<f32>;");
        self.row_loader(k, &format!("Row_{name}"), &format!("loft_rows_{name}"), &format!("loft_read_{name}"));
        self.foreign_rows.insert(name.clone());
        self.tables.insert(name.clone(), format!("loft_read_{name}"));
    }

    pub fn row_header(&mut self, k: &KindDecl) {
        // The row: the author's fields, then the compiler's hidden columns.
        let (fields, loads, stride) = self.row_parts(k, "rows");
        let pose = match pose_field(k) {
            Some(i) => format!("r.r_{}", k.row[i].name.name),
            None => "LoftXform(mat3x3<f32>(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0), vec3<f32>(0.0))".into(),
        };
        let _ = write!(
            self.out,
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
    }

    pub fn constants(&mut self, constants: &BTreeSet<String>) {
        self.locals.clear();
        for name in constants {
            if self.p.param_slot(name).is_some() { continue; }
            let decl = self.p.consts[name.as_str()];
            let t = wty(self.ty_of(&decl.value));
            let prefix = if self.motion { "motion_const" } else { "loft_const" };
            if self.p.runtime_constant(name) {
                let _ = writeln!(self.out, "fn {prefix}_{name}() -> {t} {{");
                let v = self.expr(&decl.value);
                let _ = writeln!(self.out, "  return {v};\n}}");
            } else {
                let v = self.expr(&decl.value);
                let _ = writeln!(self.out, "const {prefix}_{name}: {t} = {v};");
            }
        }
    }

    fn library(&mut self, fns: &[&FnDecl], consts: &BTreeSet<String>) {
        self.constants(consts);
        for f in fns {
            self.func(f, &fn_symbol(&f.name.name));
        }
    }
}

/// The lobe sequence of a kind's material: every `return` must build the same one (it fixes the `Bsdf` struct).
fn material_layout(p: &Prog, k: &KindDecl) -> Res<Shape> {
    fn_shape(p, k.query(QueryKind::Material).expect("checked material"), &mut BTreeSet::new())
}

const SHADE_ENTRY: &str = "// One workgroup per 8x8 tile that classify listed for this kind.";
const TWIN_BINDINGS: &str = "@group(3) @binding(0) var<storage, read> twin_in";

/// The other kinds whose rows a kind's queries read (`K[i]`), in first-use order; group 1, slot 4 + j in its module.
pub fn kind_reads(p: &Prog, k: &KindDecl) -> Vec<String> {
    let mut roots: Vec<&FnDecl> = k.queries.iter().map(|(_, f)| f).collect();
    roots.extend(k.fns.iter());
    let (lib, _) = reachable(p, &roots, &[]);
    let mut reads: Vec<String> = vec![];
    for f in roots.into_iter().chain(lib) {
        for (name, _) in References::function(f).names {
            if name != k.name.name && p.kinds.contains_key(name) && !reads.iter().any(|r| r == name) {
                reads.push(name.to_owned());
            }
        }
    }
    reads
}

fn traced_march(count_steps: bool) -> String {
    render_template(include_str!("../../../stdlib/core/march.wgsl"), &[
        ("STEPS_INIT", if count_steps { "  var steps = 0u;\n" } else { "" }),
        ("STEPS_4", if count_steps { "    steps += 1u;\n" } else { "" }),
        ("STEPS_8", if count_steps { "        steps += 1u;\n" } else { "" }),
    ])
}

/// The text of one kind's module. `light` is the library fn the shade pass calls (named in the schedule).
/// `fixed` holds kind parameters as literals; the others are read from the row.
pub fn emit_kind_module(
    p: &Prog, types: &[Ty], k: &KindDecl, light: Option<&str>, luts: &[LutRef],
    draw: Draw, aov: Aov, fixed: &BTreeMap<(String, String), String>,
) -> Res<String> {
    let mut em = Emitter::new(p, types);
    em.luts = luts;
    em.fixed = fixed;
    em.header(abi::NONCE_PLACEHOLDER);
    let q = |which| k.query(which).is_some();
    let mut roots: Vec<&FnDecl> = k.queries.iter().map(|(_, f)| f).collect();
    roots.extend(k.fns.iter());
    let mesh = matches!(draw, Draw::Mesh { .. });
    let mesh_lobe = mesh.then(|| p.linked(k.name.span, "ggx")).transpose()?;
    let mut extra: Vec<_> = light.into_iter().collect();
    extra.extend(mesh_lobe);
    let (lib, consts) = reachable(p, &roots, &extra);
    em.row_header(k);
    em.tables.insert(k.name.name.clone(), "load_row".into());
    for (j, read) in kind_reads(p, k).iter().enumerate() {
        em.foreign_table(p.kinds[read.as_str()], 1, 4 + j as u32);
    }
    // The Bsdf struct: one field per bound lobe argument, in the material's lobe order.
    let layout = if mesh { Shape::default() } else { material_layout(p, k)? };
    // A mesh's struct is its one fixed lobe (below); a mesh material written as that lobe still builds `Bsdf`.
    em.material = Some(mesh_lobe.map_or_else(|| layout.clone(), |lobe| Shape::lobe(lobe, 3)));
    let mut lobes: Vec<_> = layout.lobes.iter().map(|(name, arity)| (name.as_str(), *arity)).collect();
    if let Some(lobe) = mesh_lobe { lobes.push((lobe, 3)); }
    let invariants = lobes.iter().map(|(name, _)| {
        if light == Some("sun_sky") { lighting::invariant_lets(p, p.fns[name]) } else { Vec::new() }
    }).collect::<Vec<_>>();
    let mut bfields = vec![];
    let mut evals = String::new();
    let mut replacements = Vec::new();
    for (i, (name, arity)) in lobes.iter().enumerate() {
        let f = p.fns[name];
        let mut names = vec![];
        for (j, prm) in f.params.iter().take(*arity).enumerate() {
            bfields.push(format!("  l{i}_{j}: {},", wty(&em.resolve(prm.ty.as_ref().expect("typed")))));
            names.push(format!("b.l{i}_{j}"));
        }
        names.extend(["n".into(), "v".into(), "l".into()]);
        let args = names.join(", ");
        let call = format!("  let r{i} = {}({args});", fn_symbol(name));
        if !invariants[i].is_empty() {
            replacements.push((call.clone(), format!("  let r{i} = loft_prepared_lobe_{i}({args}, prepared);")));
        }
        let _ = writeln!(evals, "{call}");
        if layout.weights != 0 { continue; }
        if *name == "emit_lobe" {
            let _ = writeln!(evals, "  emission += r{i}.xyz;");
        } else {
            let _ = writeln!(evals, "  c = c * r{i}.w + r{i}.xyz;");
        }
    }
    let result = if layout.weights == 0 { "c + emission".into() } else {
        em.out.push_str("struct LoftMaterial { c: vec3<f32>, t: f32, e: vec3<f32> }\n");
        bfields.extend((0..layout.weights).map(|i| format!("  mix{i}: f32,")));
        layout.eval(&mut evals)
    };
    // The shelf's reflective lobes vanish behind the surface. The wrapped
    // diffuse lobe extends support only by its authored wrap. Preserve custom lobes.
    let mut horizon = "0.0".to_owned();
    let mut finite_support = true;
    for (i, (name, _)) in lobes.iter().enumerate() {
        match *name {
            "ggx" | "coat" | "film" | "lambert" | "sheen" | "emit_lobe" => {}
            "sss" => horizon = format!("max({horizon}, clamp(b.l{i}_1 * 4.0, 0.0, 1.0))"),
            _ => finite_support = false,
        }
    }
    let support = if finite_support {
        format!("  if dot(n, l) <= -{horizon} && dot(l, l) > 0.0 {{ return vec3<f32>(0.0); }}\n")
    } else { String::new() };
    let _ = write!(
        em.out,
        "// The material's lobes, bottom first; `l = 0` asks for emitted radiance.\nstruct Bsdf {{\n{}\n}}\n\
         fn eval_bsdf(b: Bsdf, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec3<f32> {{\n{support}  var c = vec3<f32>(0.0);\n  var emission = vec3<f32>(0.0);\n{evals}  return {result};\n}}\n\n",
        bfields.join("\n")
    );
    if !replacements.is_empty() {
        let mut prepared_evals = evals;
        for (call, replacement) in replacements {
            prepared_evals = prepared_evals.replace(&call, &replacement);
        }
        let body = format!("{support}  var c = vec3f(0.0);\n  var emission = vec3f(0.0);\n{prepared_evals}  return {result};\n");
        lighting::emit_prepared(&mut em, &lobes, &invariants, &body);
        em.prepare_light = true;
    }
    em.library(&lib, &consts);
    for f in &k.fns {
        em.func(f, "loft_surface_jvp");
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
    em.sampled_textures();
    // Draw-form glue: the vis entry points, how shade rebuilds the hit, and the twin's geometry.
    let count_steps = aov == Aov::SdfSteps && draw == Draw::Traced;
    let uses_world_hit = light.is_none() && aov == Aov::Beauty;
    let (mut vis, mut geometry, twin_geo) = match draw {
        Draw::Surface { nu, nv } => (
            render_template(include_str!("../../../stdlib/core/surface_vis.wgsl"), &[("NU", &nu.to_string()), ("NV", &nv.to_string())]),
            include_str!("../../../stdlib/core/surface_geometry.wgsl").to_owned(),
            include_str!("../../../stdlib/core/surface_twin.wgsl").to_owned(),
        ),
        Draw::Mesh { .. } => crate::mesh::draw(),
        Draw::Analytic | Draw::Traced => {
            // World shading reconstructs the point using the same pixel ray and
            // near offset as traversal. Proxy interpolation rounds that ray
            // differently, which can move a secondary origin across a surface.
            let primary_direction = if uses_world_hit { "loft_ray(v.pos.xy)" } else { "normalize(v.wp - ro)" };
            let (hit, split_ao) = match draw {
                Draw::Analytic if uses_world_hit => (r"  let near = select(0.0001, 0.0, frame.cold.y == 1u);
  let local_t = q_visible(row, loft_xf_pt(xf, ro + near * rd), lrd);
  if local_t < 0.0 { discard; }
  let t = near + local_t;
".to_owned(), false),
                Draw::Analytic => ("  // Closed form: the author's `visible` query, in the kind's space (a rigid pose keeps t).\n  let t = q_visible(row, lro, lrd);\n  if t < 0.0 { discard; }\n".to_owned(),
                    light == Some("woodblock_pigment") && aov == Aov::Beauty),
                Draw::Traced => (traced_march(count_steps), false),
                Draw::Surface { .. } | Draw::Mesh { .. } => unreachable!("ray draw"),
            };
            // Diagnostic-only Vis word: 24 high distance bits and the 8-bit step count.
            let distance = if count_steps { "((bitcast<u32>(t) & 0xffffff00u) | steps)" } else { "bitcast<u32>(t)" };
            let visible_twin = if q(QueryKind::Visible) { "q_visible(row, pl, dl)" } else { "0.0" };
            (
                render_template(include_str!("../../../stdlib/core/ray_vis.wgsl"), &[("PRIMARY_DIRECTION", primary_direction), ("HIT", &hit), ("DISTANCE", distance)]),
                if split_ao {
                    // The tetrahedral sum and the five AO taps keep their original
                    // order. Separate the loops to compute only the active sample.
                    include_str!("../../../stdlib/core/ray_geometry_split_ao.wgsl").to_owned()
                } else {
                    include_str!("../../../stdlib/core/ray_geometry.wgsl").to_owned()
                },
                render_template(include_str!("../../../stdlib/core/ray_twin.wgsl"), &[("VISIBLE_TWIN", visible_twin)]),
            )
        }
    };
    let mut twin_geo = twin_geo;
    if matches!(draw, Draw::Surface { .. }) && k.proof != GeometryProof::Required {
        // Four diagonal taps recover the two tangents of the parameterized surface.
        let step = FINITE_DIFF_STEP;
        let stencil = |uv: &str| format!(
            "let a0 = q_surface(row, {uv}.x - {step}, {uv}.y - {step});\n  \
             let a1 = q_surface(row, {uv}.x + {step}, {uv}.y - {step});\n  \
             let a2 = q_surface(row, {uv}.x - {step}, {uv}.y + {step});\n  \
             let a3 = q_surface(row, {uv}.x + {step}, {uv}.y + {step});\n  \
             let cr = cross((a1 - a0) + (a3 - a2), (a2 - a0) + (a3 - a1));");
        geometry = geometry.replace(
            "let cr = cross(loft_surface_jvp(row, uv.x, uv.y, 1.0, 0.0), loft_surface_jvp(row, uv.x, uv.y, 0.0, 1.0));",
            &stencil("uv"));
        twin_geo = twin_geo.replace(
            "let cr = cross(loft_surface_jvp(row, a.x, a.y, 1.0, 0.0), loft_surface_jvp(row, a.x, a.y, 0.0, 1.0));",
            &stencil("a"));
    }
    let mut diagnostics = "";
    if let Draw::Surface { nu, nv } = draw {
        vis.push_str(&surface::indexed_grid(p, k.query(QueryKind::Surface).expect("surface query"), nu, nv));
    }
    let shade: String = match aov {
        // The shelf light evaluates an emission-only stack exactly once at zero incident direction.
        Aov::Beauty if light == Some("sun_sky") && layout.lobes.iter().all(|(name, _)| *name == "emit_lobe") =>
            "eval_bsdf(b, n, -rd, vec3<f32>(0.0))".into(),
        Aov::Beauty => match light {
            Some(light) => format!("{}(b, n, -rd, sun, loft_shadow(p, n), ao)", fn_symbol(light)),
            // The tiled World module replaces this entry before validation.
            None => "vec3<f32>(0.0)".into(),
        },
        Aov::Albedo => layout.lobes.iter().enumerate().find_map(|(i, (name, _))| {
            let f = p.fns[name.as_str()];
            f.params.iter().take(f.params.len().saturating_sub(3))
                .position(|p| p.name.name == "albedo").map(|j| format!("b.l{i}_{j}"))
        }).unwrap_or_else(|| "vec3<f32>(0.0)".into()),
        Aov::Normal => "n * 0.5 + 0.5".into(),
        Aov::Depth => "vec3<f32>(min(log2(1.0 + distance(p, frame.eye.xyz)) / 10.0, 254.0 / 255.0))".into(),
        Aov::Id => {
            diagnostics = aov::ID_HASH;
            "loft_id(v.x)".into()
        }
        Aov::SdfSteps => {
            diagnostics = aov::TRACE_STEPS;
            if count_steps {
                geometry = geometry.replace("bitcast<f32>(v.y)", "bitcast<f32>(v.y & 0xffffff00u)");
                "loft_steps(f32(v.y & 255u) / 192.0)".into()
            } else {
                "loft_steps(0.0)".into()
            }
        }
        Aov::LipschitzRatio => {
            diagnostics = aov::LIPSCHITZ_RATIO;
            if matches!(draw, Draw::Surface { .. }) {
                "vec3<f32>(0.0)".into()
            } else {
                // Axis differences estimate the direction. One secant in that direction is
                // itself bounded by L, even where axis samples straddle different smooth pieces.
                geometry.push_str(include_str!("../../../stdlib/core/ray_lipschitz.wgsl"));
                "loft_ratio(ratio)".into()
            }
        }
        Aov::Overdraw => {
            diagnostics = aov::OVERDRAW;
            "vec3<f32>(0.0)".into()
        }
    };
    // Module-scope declarations are order-free in WGSL. The composed Bsdf structs go before the shade entry,
    // since the path and tiled integrators embed only the text above it (namespacing each `struct` per kind).
    let structs = std::mem::take(&mut em.bsdf_structs);
    em.out.extend(structs.into_values());
    let [l0, l1, l2] = unit(TWIN_LIGHT).map(|v| f32_lit(f64::from(v)));
    em.out += &render_template(include_str!("../../../stdlib/core/kind_shade.wgsl"), &[
        ("QUERIES_END", paths::QUERIES_END), ("VIS", &vis), ("DIAGNOSTICS", diagnostics),
        ("SHADOW", include_str!("../../../stdlib/core/shadow.wgsl")), ("SHADE_ENTRY", SHADE_ENTRY), ("GEOMETRY", &geometry),
        ("SHADE", &shade), ("TWIN_BINDINGS", TWIN_BINDINGS), ("TWIN_GEO", &twin_geo),
        ("L0", &l0), ("L1", &l1), ("L2", &l2),
    ]);
    Ok(em.out)
}

/// Scan tables: frame `i` of a scan at `off`, clamped to its ends; the uniform cubic B-spline (C2 in t) and its
/// t-derivative scaled by `dt`, the same weights the CPU evaluator uses.
const LUTS: &str = concat!(include_str!("../../../stdlib/core/luts.wgsl"), "\n");

/// A frame pass a schedule wrote (`loft-passes/src/frame_pass.rs`): the frame prelude, then, when a hole names Loft
/// functions, the helpers and library they reach, then the filled template. The stock cull, classify and present
/// modules are these.
pub fn template_module(p: &Prog, types: &[Ty], functions: &[&str], body: &str) -> String {
    if functions.is_empty() {
        return render_template(PRELUDE, &[("NONCE", abi::NONCE_PLACEHOLDER)]) + body;
    }
    let mut em = Emitter::new(p, types);
    em.header(abi::NONCE_PLACEHOLDER);
    let (lib, consts) = reachable(p, &[], functions);
    em.library(&lib, &consts);
    em.out + body
}

pub fn bake_module(p: &Prog, types: &[Ty], decl: &BakeDecl) -> Res<(crate::bake_tex::Clock, u32, String)> {
    use crate::bake_tex::{Clock, size, workgroup};
    let mut em = Emitter::new(p, types);
    let (coord, storage, index, arg) = match decl.dim {
        TexDim::D2(..) => (Ty::VecN(2), "2d", "vec2<i32>(id.xy)", "u.xy"),
        TexDim::D3(..) => (Ty::VecN(3), "3d", "vec3<i32>(id)", "u"),
        TexDim::Cube(_) => (Ty::Geo(Geo::Dir, Space::Named("World".into())), "2d_array", "vec2<i32>(id.xy), i32(id.z)", "bake_direction(id.z, u.xy)"),
    };
    let prm = &decl.texel.params[0];
    if em.resolve(prm.ty.as_ref().expect("typed bake coordinate")) != coord {
        return Err(Diag::new(Code::E003, prm.name.span, "bake coordinate has the wrong type", "use v2 for 2D, v3 for 3D or d3@World for cube"));
    }
    let clock = match &decl.clock {
        BakeClock::Load => Clock::Load,
        BakeClock::Dirty(names) => {
            let mut words = BTreeSet::new();
            for name in names {
                let word = p.param_word(&name.name).ok_or_else(|| Diag::new(Code::E011, name.span,
                    "dirty bake watches an unknown live parameter", "name a live scalar param"))?;
                if !words.insert(word) {
                    return Err(Diag::new(Code::E011, name.span, "duplicate dirty parameter", "watch each parameter once"));
                }
            }
            Clock::Dirty(words.into_iter().collect())
        }
    };
    em.header(abi::NONCE_PLACEHOLDER);
    let (functions, constants) = reachable(p, &[&decl.texel], &[]);
    em.library(&functions, &constants);
    let column = if matches!(decl.dim, TexDim::D2(_, h) if h >= 64) {
        column::emit(&mut em, &decl.texel)
    } else { None };
    let rows_per_invocation = if column.is_some() { 4 } else { 1 };
    em.baked_column = column;
    em.func(&decl.texel, "texel_value");
    if let TexDim::Cube(_) = decl.dim {
        em.out.push_str(include_str!("../../../stdlib/core/bake_cube.wgsl").strip_suffix('\n').unwrap());
    }
    let [x, y, z] = size(decl.dim);
    let [wx, wy, wz] = workgroup(decl.dim);
    let encoding = match decl.format.name.as_str() {
        "r8unorm" | "r16float" | "r32float" => "vec4<f32>(texel, 0.0, 0.0, 1.0)",
        _ => "texel",
    };
    let format_name = &decl.format.name;
    let (invocation, column_arg, end) = if column.is_some() {
        (render_template(include_str!("../../../stdlib/core/bake_column.wgsl").strip_suffix('\n').unwrap(), &[("ROWS_PER_INVOCATION", &rows_per_invocation.to_string())]), ", column", "  }")
    } else {
        (include_str!("../../../stdlib/core/bake_invocation.wgsl").strip_suffix('\n').unwrap().to_owned(), "", "")
    };
    em.out += &render_template(include_str!("../../../stdlib/core/bake.wgsl"), &[
        ("STORAGE", storage), ("FORMAT_NAME", format_name),
        ("WX", &wx.to_string()), ("WY", &wy.to_string()), ("WZ", &wz.to_string()),
        ("X", &x.to_string()), ("Y", &y.to_string()), ("Z", &z.to_string()),
        ("INVOCATION", &invocation), ("ARG", arg), ("COLUMN_ARG", column_arg),
        ("INDEX", index), ("ENCODING", encoding), ("END", end),
    ]);
    Ok((clock, rows_per_invocation, em.out))
}

fn atomic_target(expr: &Expr) -> Option<(&str, &Expr, &str)> {
    let ExprKind::Member(base, field) = &expr.kind else { return None; };
    let ExprKind::Index(table, index) = &base.kind else { return None; };
    let ExprKind::Name(table) = &table.kind else { return None; };
    Some((table, index, &field.name))
}

/// Float table fields keep their f32 representation at every pass boundary.
#[derive(Clone, Copy)]
pub enum FloatAtomics {
    Native,
    Fixed(u32),
}

#[derive(Clone, Copy)]
enum AtomicScalar {
    I32,
    U32,
    Float(FloatAtomics),
}

fn float_atomic_symbol(table: &str, field: &str) -> String {
    format!("loft_atomic_add_{}_{table}_{field}", table.len())
}

fn storage_fields<'a>(p: &Prog<'a>, row: &str) -> &'a [RowField] {
    if let Some(k) = p.kinds.get(row) { &k.row } else { p.structs[row].1 }
}

fn table_struct(em: &mut Emitter<'_, '_>, symbol: &str, row: &str, parameter: Option<&str>) {
    let _ = writeln!(em.out, "struct {symbol} {{");
    for field in storage_fields(em.p, row) {
        let name = &field.name.name;
        let field_ty = em.resolve(&field.ty);
        let atomic = parameter.and_then(|p| em.kernel_atomics.get(&(p.to_owned(), name.clone())));
        let ty = match atomic {
            Some(AtomicScalar::U32) => "atomic<u32>".to_owned(),
            Some(AtomicScalar::I32 | AtomicScalar::Float(FloatAtomics::Fixed(_))) => "atomic<i32>".to_owned(),
            Some(AtomicScalar::Float(FloatAtomics::Native)) => "atomic<f32>".to_owned(),
            None => wty(&field_ty),
        };
        let name = if em.p.kinds.contains_key(row) { format!("r_{name}") } else { member_name(name) };
        let _ = writeln!(em.out, "  {name}: {ty},");
    }
    em.out.push_str("}\n");
}

fn nested_table_structs(em: &mut Emitter<'_, '_>, rows: &[&str]) {
    let mut emitted = BTreeSet::new();
    let mut pending = rows.to_vec();
    while let Some(row) = pending.pop() {
        for field in storage_fields(em.p, row) {
            let mut ty = em.resolve(&field.ty);
            while let Ty::Array(element, _) = ty { ty = *element; }
            if let Ty::Struct(name) = ty && let Some((&name, _)) = em.p.structs.get_key_value(name.as_str())
                && name != "World" && emitted.insert(name)
            {
                pending.push(name);
            }
        }
    }
    for name in emitted {
        table_struct(em, &format!("loft_type_{name}"), name, None);
    }
}

pub fn table_layout_source(p: &Prog<'_>, row: &str) -> String {
    let mut em = Emitter::new(p, &[]);
    em.header(abi::NONCE_PLACEHOLDER);
    nested_table_structs(&mut em, &[row]);
    table_struct(&mut em, "LoftTableRow", row, None);
    em.out.push_str("@group(1) @binding(0) var<storage, read> loft_table: array<LoftTableRow>;");
    em.out
}

pub fn kernel_uses_float(k: &KernelDecl, types: &[Ty]) -> bool {
    References::function(&k.f).atomic_targets.iter().any(|target| types[target.id as usize] == Ty::F32)
}

pub fn kernel_module(
    p: &Prog<'_>, types: &[Ty], k: &KernelDecl,
    arguments: &[String], bindings: &[(&str, &str, loft_ir::PassBinding, u32)], workgroup: [u32; 3],
    float_atomics: FloatAtomics,
) -> (String, Vec<loft_ir::PassBinding>) {
    let mut em = Emitter::new(p, types);
    em.kernel_sampling = true;
    em.header(abi::NONCE_PLACEHOLDER);
    for target in References::function(&k.f).atomic_targets {
        let (table, _, field) = atomic_target(target).expect("checked atomic destination");
        let scalar = match em.ty_of(target) {
            Ty::U32 => AtomicScalar::U32,
            Ty::I32 => AtomicScalar::I32,
            Ty::F32 => AtomicScalar::Float(float_atomics),
            _ => unreachable!("checked atomic scalar"),
        };
        em.kernel_atomics.insert((table.to_owned(), field.to_owned()), scalar);
    }
    let (mut encode, mut decode, mut converted) = (String::new(), String::new(), Vec::new());
    let (helpers, consts) = reachable(p, &[&k.f], &[]);
    nested_table_structs(&mut em, &bindings.iter().map(|(_, row, _, _)| *row).collect::<Vec<_>>());
    em.library(&helpers, &consts);
    for (param, row, binding, capacity) in bindings {
        let ty = format!("LoftTable_{param}");
        table_struct(&mut em, &ty, row, Some(param));
        let access = if binding.access == loft_ir::BindAccess::ReadStorage { "read" } else { "read_write" };
        let resource = wgsl_name(param);
        let _ = writeln!(em.out, "@group({}) @binding({}) var<storage, {access}> {resource}: array<{ty}>;",
            binding.group, binding.slot);
        let mut conversions = Vec::new();
        for ((table, field), kind) in &em.kernel_atomics {
            if table != param || !matches!(kind, AtomicScalar::Float(FloatAtomics::Fixed(_))) { continue; }
            let function = float_atomic_symbol(table, field);
            let field = if p.kinds.contains_key(row) { format!("r_{field}") } else { member_name(field) };
            em.out += &render_template(include_str!("../../../stdlib/core/kernel_atomic.wgsl"), &[
                ("FUNCTION", &function), ("RESOURCE", &resource), ("FIELD", &field),
            ]);
            conversions.push(field);
        }
        if !conversions.is_empty() {
            converted.push(*binding);
            // A separate dispatch changes representation before accumulation. Use
            // logical capacity because a backend may pad the storage allocation.
            for body in [&mut encode, &mut decode] {
                let _ = writeln!(body, "for (var i = gid.x; i < {capacity}u; i += groups.x * 64u) {{");
            }
            for field in conversions {
                let ptr = format!("&{resource}[i].{field}");
                let _ = writeln!(encode, "atomicStore({ptr}, loft_fixed_encode(bitcast<f32>(atomicLoad({ptr}))));");
                let _ = writeln!(decode, "atomicStore({ptr}, bitcast<i32>(loft_fixed_decode(atomicLoad({ptr}))));");
            }
            encode.push_str("}\n");
            decode.push_str("}\n");
        }
    }
    if let FloatAtomics::Fixed(scale) = float_atomics && !converted.is_empty() {
        em.out += &render_template(include_str!("../../../stdlib/core/kernel_fixed.wgsl"), &[
            ("SCALE", &scale.to_string()), ("ENCODE", &encode), ("DECODE", &decode),
        ]);
    }
    let mut f = k.f.clone();
    f.params.retain(|p| !matches!(p.ty, Some(TyExpr::Table(_))));
    em.out.push_str("var<private> loft_gid: vec3<u32>;\n");
    em.func(&f, "loft_kernel");
    em.sampled_textures();
    let [x, y, z] = workgroup;
    em.out += &render_template(include_str!("../../../stdlib/core/kernel.wgsl"), &[
        ("X", &x.to_string()), ("Y", &y.to_string()), ("Z", &z.to_string()),
        ("ARGS", &arguments.join(", ")),
    ]);
    (em.out, converted)
}
