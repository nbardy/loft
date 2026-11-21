mod assemble;
mod lower_passes;
mod schedule;
mod stage0;

use crate::{CompileOptions, Compiled, Prog, Report, check_modules, eval, gltf, parse_all};
use crate::program::Program;
use crate::source::Graph;
use loft_eval::{as_f32, refine};
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;
use std::time::Instant;

struct Timings {
    clock: Instant,
    ms: Vec<(&'static str, f64)>,
}

impl Timings {
    fn lap(&mut self, name: &'static str) {
        self.ms.push((name, self.clock.elapsed().as_secs_f64() * 1e3));
        self.clock = Instant::now();
    }
}

pub(super) fn compile(src: &Graph, opts: &CompileOptions<'_>) -> Res<(Program, Report, Compiled)> {
    let mut timings = Timings { clock: Instant::now(), ms: vec![] };
    let mut modules = parse_all(src)?;
    let meshes = gltf::lower(src, &mut modules)?;
    apply_overrides(&mut modules, opts.overrides)?;
    timings.lap("parse+ad");

    let mut types = vec![];
    let pending_lowering = check_modules(&mut modules, &mut types)?;
    let p = Prog::index(&modules)?;
    check_scene(&p, &types, opts.overrides, pending_lowering)?;
    timings.lap("check");
    let mut schedule = schedule::resolve(&p, &modules, &mut types, src, opts)?;
    timings.lap("schedule");
    let mut scene = stage0::evaluate(&p, &modules, &mut types, src, opts, &meshes, &mut schedule, &mut timings)?;
    let lowered = lower_passes::lower(&p, &modules, &mut types, src, opts, &mut schedule, &mut scene, &mut timings)?;
    let assembled = assemble::assemble(&p, &types, schedule, scene, lowered, timings.ms)?;
    Ok(assembled.finish(modules, types, meshes))
}

fn apply_overrides(modules: &mut [Module], overrides: &[(String, f64)]) -> Res<()> {
    for c in modules.iter_mut().flat_map(|m|&mut m.items).filter_map(|it|if let Item::Const(c)=it{Some(c)}else{None}){
        if c.comptime && let Some((_,v))=overrides.iter().find(|(n,_)|n==&c.name.name){
            let integer=matches!(&c.ty,Some(TyExpr::Name(n)) if n.name=="u32" || n.name=="i32");
            if !v.is_finite() || integer && (v.fract()!=0. || matches!(&c.ty,Some(TyExpr::Name(n)) if n.name=="u32") && !(0.0..=u32::MAX as f64).contains(v) || matches!(&c.ty,Some(TyExpr::Name(n)) if n.name=="i32") && !(i32::MIN as f64..=i32::MAX as f64).contains(v)){
                return Err(Diag::new(Code::E602,c.name.span,"invalid parameter override","supply a finite value in the parameter type's domain"))
            }
            c.value.kind=ExprKind::Num{value:*v,float:!integer,text:v.to_string(),suffix:NumSuffix::None};
        }
    }
    Ok(())
}

fn check_scene(p: &Prog<'_>, types: &[Ty], overrides: &[(String, f64)], pending_lowering: Option<Span>) -> Res<()> {
    for (name, _) in overrides {
        match p.consts.get(name.as_str()) {
            Some(c) if c.comptime => {}
            _ => {
                return Err(Diag::new(Code::E011, Span::NONE, format!("`--set {name}`: no scene parameter with this name"), "override a declared `param`"));
            }
        }
    }
    for c in p.consts.values(){
        if let Some(range)=&c.range{
            let mut ev: eval::Eval<f32> = eval::Eval::new(p, types, vec![], overrides);
            let provided=overrides.iter().find(|(n,_)|n==&c.name.name).map(|(_,v)|*v as f32);
            let x=match provided{Some(v)=>v,None=>as_f32(eval::concrete(&ev.expr(&c.value)?,c.value.span)?,c.value.span)?};
            ev.validate_range(&c.name,range,x,c.value.span)?;
        }
    }
    refine::check(p,types,overrides)?;
    if let Some(span) = pending_lowering {
        return Err(Diag::new(Code::E405, span, "Phase 4 lowering is pending for this checked program",
            "use the typed front end to inspect the Phase 4 contract"));
    }
    Ok(())
}
