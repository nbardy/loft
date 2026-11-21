//! Infer private signatures from calls and returns, before differentiating them.
//! Each successful iteration fills at least one hole. The ordinary checker
//! subsequently checks every call against the same generalized signature.
use crate::{Prog, check::Ck};
use loft_ir::{Res, Span};
use loft_parse::ast::*;
use std::collections::{BTreeMap, HashMap};

#[derive(Default)]
pub(crate) struct Hints {
    pub args: BTreeMap<String, Vec<Ty>>,
    pub returns: BTreeMap<String, Ty>,
    pub current: Option<String>,
}
fn syntax(
    t: &Ty,
    sp: Span,
    spaces: &mut HashMap<Space, String>,
    generalize: bool,
) -> Option<TyExpr> {
    let n = |name: &str| Ident {
        name: name.into(),
        span: sp,
    };
    let mut space = |s: &Space| -> Option<SpaceRef> {
        let (name, var) = match s {
            Space::Var(n) => (n.clone(), true),
            s if generalize => {
                let len = spaces.len();
                (
                    spaces
                        .entry(s.clone())
                        .or_insert_with(|| format!("S{len}"))
                        .clone(),
                    true,
                )
            }
            Space::Named(n) => (n.clone(), false),
            Space::Fresh(..) | Space::Open => return None,
        };
        Some(SpaceRef {
            name,
            var,
            span: sp,
        })
    };
    Some(match t {
        Ty::F32 | Ty::AbstractInt => TyExpr::Name(n("f32")),
        Ty::I32 => TyExpr::Name(n("i32")),
        Ty::U32 => TyExpr::Name(n("u32")),
        Ty::Bool => TyExpr::Name(n("bool")),
        Ty::VecN(k) => TyExpr::Name(n(&format!("vec{k}f"))),
        Ty::Geo(g, s) => TyExpr::Geo(*g, space(s)?),
        Ty::Frame(s) => TyExpr::Frame(space(s)?),
        Ty::Xform(a, b) => TyExpr::Xform(space(a)?, space(b)?),
        Ty::Bsdf => TyExpr::Name(n("Bsdf")),
        Ty::Row(k) | Ty::Id(k) | Ty::Struct(k) => TyExpr::Name(n(k)),
        Ty::Array(t, n) => TyExpr::Array(Box::new(syntax(t, sp, spaces, generalize)?), *n, sp),
        Ty::Table(k) => TyExpr::Table(n(k)),
        Ty::Map(..) | Ty::Verdict | Ty::Void => return None,
    })
}

pub(crate) fn run(modules: &mut [Module]) -> Res<()> {
    loop {
        let p = Prog::index(modules)?;
        if p.fns
            .values()
            .all(|f| f.params.iter().all(|a| a.ty.is_some()) && (f.ret.is_some() || f.comptime))
        {
            return Ok(());
        }
        let mut types = vec![];
        let mut hints = Hints::default();
        let mut ck = Ck::new(&p, &mut types);
        ck.hints = Some(&mut hints);
        // A blocked site is revisited once its callees have types. Final check
        // always runs and diagnoses any unsolved holes at the author's span.
        for f in p.fns.values() {
            let _ = ck.func(f, None);
        }
        for k in p.kinds.values() {
            for (query, f) in &k.queries {
                let _ = ck.query(k, *query, f);
            }
        }
        for sc in p.scans.values() {
            let _ = ck.scan(sc);
        }
        for k in p.kernels.values() { let _ = ck.kernel(k); }
        for law in p.laws.values() {
            let _ = ck.motion(&law.f, Some(&law.writes));
        }
        let mut progress = false;
        for item in modules.iter_mut().flat_map(|m| &mut m.items) {
            let Item::Fn(f) = item else { continue };
            let mut spaces = HashMap::new();
            if let Some(args) = hints.args.get(&f.name.name) {
                for (p, t) in f.params.iter_mut().zip(args) {
                    if p.ty.is_none() {
                        p.ty = syntax(t, p.name.span, &mut spaces, true);
                        progress |= p.ty.is_some();
                    }
                }
            }
            if f.ret.is_none()
                && let Some(t) = hints.returns.get(&f.name.name)
            {
                f.ret = syntax(t, f.name.span, &mut spaces, false);
                progress |= f.ret.is_some();
            }
        }
        if !progress {
            return Ok(());
        }
    }
}
