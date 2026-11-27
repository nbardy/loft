use crate::Prog;
use loft_ir::Span;
use loft_parse::ast::*;
use std::collections::BTreeSet;

pub fn fn_symbol(n: &str) -> String {
    match n.split_once('#') {
        Some(("", internal)) => format!("loft_internal_{internal}"),
        Some((kind, helper)) => format!("loft_kind_{}_{}_{}", kind.len(), kind, helper),
        None => format!("loft_user_{n}"),
    }
}

#[derive(Default)]
pub struct References<'a> {
    proof_only: bool,
    row_reads: Option<BTreeSet<&'a str>>,
    pub names: Vec<(&'a str, Span)>,
    pub calls: Vec<&'a Ident>,
    pub scans: BTreeSet<&'a str>,
    pub step_scales: Vec<&'a Expr>,
    pub texture_names: BTreeSet<&'a str>,
    pub atomic_targets: Vec<&'a Expr>,
}
impl<'a> References<'a> {
    pub fn function(f: &'a FnDecl) -> Self {
        Self::from_body(f, false)
    }

    fn from_body(f: &'a FnDecl, proof_only: bool) -> Self {
        let mut refs = Self { proof_only, ..Self::default() };
        for default in f.params.iter().filter_map(|p| p.default.as_ref()) {
            refs.expr(default, &[]);
        }
        let mut locals = f.params.iter().map(|p| p.name.name.as_str()).collect();
        refs.block(&f.body, &mut locals);
        if let Some(raw) = &f.raw { refs.calls.extend(&raw.references); }
        refs
    }

    pub fn expr(&mut self, e: &'a Expr, locals: &[&'a str]) {
        match &e.kind {
            ExprKind::Unproved { step_scale, body } => {
                self.step_scales.push(step_scale);
                self.expr(step_scale, locals);
                if !self.proof_only {
                    let mut inner = locals.to_vec();
                    self.statements(&body.body, &mut inner);
                    self.expr(&body.value, &inner);
                }
            }
            ExprKind::FiniteDiff { function, args, .. } => {
                self.calls.push(function);
                self.args(args, locals);
            }
            ExprKind::If { condition, then, otherwise } => {
                self.expr(condition, locals);
                for arm in [then, otherwise] {
                    let mut arm_locals = locals.to_vec();
                    self.statements(&arm.body, &mut arm_locals);
                    self.expr(&arm.value, &arm_locals);
                }
            }
            ExprKind::Call { callee, args } => {
                self.calls.push(callee);
                if callee.name == "atomic_add" && let Some(a) = args.first() {
                    self.atomic_targets.push(&a.value);
                }
                if let Some(Arg { value: Expr { kind: ExprKind::Name(n), .. }, .. }) = args.first() {
                    match callee.name.as_str() {
                        "sample_c2" | "loft_dsample" => { self.scans.insert(n); }
                        "sample" | "sample_lod" => { self.texture_names.insert(n); }
                        _ => {}
                    }
                }
                self.args(args, locals);
            }
            ExprKind::Ctor { args, .. } => self.args(args, locals),
            ExprKind::Member(base, member) if self.row_reads.is_some()
                && matches!(&base.kind, ExprKind::Name(n) if n == "self") => {
                self.row_reads.as_mut().unwrap().insert(&member.name);
            }
            ExprKind::Name(n) if n == "self" && self.row_reads.is_some() => {
                // Passing the row to a helper lets it read any column.
                self.row_reads.as_mut().unwrap().insert("self");
            }
            ExprKind::Name(n) if !locals.contains(&n.as_str()) => self.names.push((n, e.span)),
            ExprKind::Name(_) | ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::String(_) => {}
            ExprKind::Member(b, _) | ExprKind::Unary(_, b) => self.expr(b, locals),
            ExprKind::Index(a, b) | ExprKind::Binary(_, a, b) => {
                self.expr(a, locals);
                self.expr(b, locals);
            }
        }
    }

    fn args(&mut self, args: &'a [Arg], locals: &[&'a str]) {
        for arg in args { self.expr(&arg.value, locals); }
    }

    /// The emitter leaves the row parameter out of locals to detect row-dependent surfaces.
    pub fn block(&mut self, b: &'a Block, locals: &mut Vec<&'a str>) {
        let mark = locals.len();
        self.statements(b, locals);
        locals.truncate(mark);
    }

    fn statements(&mut self, b: &'a Block, locals: &mut Vec<&'a str>) {
        for s in &b.stmts {
            match s {
                Stmt::Let { name, value, .. } => {
                    self.expr(value, locals);
                    locals.push(&name.name);
                }
                Stmt::Assign { target, value, .. } => {
                    if self.row_reads.is_none() { self.expr(target, locals); }
                    self.expr(value, locals);
                }
                Stmt::Map { name, head, stages, .. } => {
                    self.expr(head, locals);
                    for e in stages.iter().flat_map(|(_, args)| args) { self.expr(e, locals); }
                    locals.push(&name.name);
                }
                Stmt::Return(v, _) => v.iter().for_each(|e| self.expr(e, locals)),
                Stmt::If { cond, then, otherwise } => {
                    self.expr(cond, locals);
                    self.block(then, locals);
                    otherwise.iter().for_each(|o| self.block(o, locals));
                }
                Stmt::Each { var, values, body, .. } => {
                    for e in values { self.expr(e, locals); }
                    locals.push(&var.name);
                    self.block(body, locals);
                    locals.pop();
                }
                Stmt::For { var, lo, hi, body, .. } => {
                    self.expr(lo, locals);
                    self.expr(hi, locals);
                    locals.push(&var.name);
                    self.block(body, locals);
                    locals.pop();
                }
                Stmt::Emit { .. } if self.row_reads.is_some() => {}
                Stmt::Emit { fields, bind, .. } => {
                    for (_, e) in fields { self.expr(e, locals); }
                    if let Some(n) = bind { locals.push(&n.name); }
                }
                Stmt::ImportTable { index, .. } => self.expr(index, locals),
                Stmt::Expr(e) => self.expr(e, locals),
            }
        }
    }
}

/// Fields read by the trace, visible and surface queries, in row layout order.
pub fn geometry_fields(kind: &KindDecl) -> Vec<usize> {
    let mut refs = References { row_reads: Some(BTreeSet::new()), ..References::default() };
    for (_, query) in kind.queries.iter().filter(|(q, _)| *q != QueryKind::Material) {
        refs.block(&query.body, &mut vec![]);
    }
    let reads = refs.row_reads.unwrap();
    kind.row.iter().enumerate().filter_map(|(i, field)|
        (reads.contains("self") || reads.contains(field.name.name.as_str())).then_some(i)
    ).collect()
}

/// The fns, user and library, reachable from a kind's queries and the shade glue's light fn.
pub fn reachable<'a>(p: &Prog<'a>, roots: &[&'a FnDecl], extra: &[&str]) -> (Vec<&'a FnDecl>, BTreeSet<String>) {
    reachable_impl(p, roots, extra, false)
}

pub fn proof_dependencies<'a>(p: &Prog<'a>, root: &'a FnDecl) -> Vec<&'a FnDecl> {
    reachable_impl(p, &[root], &[], true).0
}

fn reachable_impl<'a>(p: &Prog<'a>, roots: &[&'a FnDecl], extra: &[&str], proof_only: bool) -> (Vec<&'a FnDecl>, BTreeSet<String>) {
    let mut seen: Vec<&FnDecl> = vec![];
    let mut consts = BTreeSet::new();
    let mut todo: Vec<&FnDecl> = roots.to_vec();
    for f in extra.iter().filter_map(|n| p.fns.get(n).copied()) {
        seen.push(f);
        todo.push(f);
    }
    while let Some(f) = todo.pop() {
        let mut refs = References::from_body(f, proof_only);
        while let Some((n, _)) = refs.names.pop() {
            if let Some(c) = p.consts.get(n) && consts.insert(n.to_owned()) {
                refs.expr(&c.value, &[]);
            }
        }
        for c in refs.calls {
            let resolved = p.fns.get(c.name.as_str()).or_else(|| {
                if c.name != "sky" { return None; }
                let key = p.linked(c.span, "sky_dome").ok()?;
                p.fns.get(key)
            });
            if let Some(g) = resolved
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
