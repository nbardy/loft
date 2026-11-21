//! Keep binary32 products observable before an ordinary sum can contract them.
use std::{collections::BTreeMap, fmt::Write};

use loft_ir::{Code, Diag, Res, Span};
use naga::{Block, Expression as E, Function, Handle, MathFunction as M, Scalar, Statement as S, TypeInner as T};
use naga::valid::{Capabilities, FunctionInfo, ValidationFlags, Validator};

const PREFIX: &str = "loft_motion_round_";
type Calls = BTreeMap<Handle<E>, (Handle<Function>, Vec<Handle<E>>)>;

fn helpers() -> String {
    // A legal row address fits in 31 bits (even a one-word row would otherwise
    // exceed the storage binding limit). Metal must still observe the integer
    // operation because the shader argument itself has all 32 bits.
    let mut wgsl = String::new();
    for n in 1..=4 {
        let ty = |n, s| if n == 1 { format!("{s}32") } else { format!("vec{n}<{s}32>") };
        let f = ty(n, "f");
        let u = ty(n, "u");
        let _ = writeln!(wgsl, "fn {PREFIX}mul_{n}(a:{f}, b:{f})->{f} {{
    return bitcast<{f}>(bitcast<{u}>(a * b) ^ {u}(loft_advance_args.first_row & 0x80000000u));
}}");
        for m in [1, n] {
            let t = ty(m, "f");
            let _ = writeln!(wgsl, "fn {PREFIX}mix_{n}_{m}(a:{f}, b:{f}, t:{t})->{f} {{
    return {PREFIX}mul_{n}(a, {f}(1.0) - {f}(t)) + {PREFIX}mul_{n}(b, {f}(t));
}}");
            if m == n { break; }
        }
        if n == 1 {
            wgsl.push_str("fn loft_motion_round_smooth_1(lo:f32, hi:f32, x:f32)->f32 {
    let t = clamp((x-lo)/(hi-lo), 0.0, 1.0);
    return loft_motion_round_mul_1(loft_motion_round_mul_1(t,t), 3.0-loft_motion_round_mul_1(2.0,t));
}\n");
            continue;
        }
        let _ = writeln!(wgsl, "fn {PREFIX}scale_{n}(a:{f}, b:f32)->{f} {{ return {PREFIX}mul_{n}(a, {f}(b)); }}
fn {PREFIX}scale_left_{n}(a:f32, b:{f})->{f} {{ return {PREFIX}mul_{n}({f}(a), b); }}");
        let sum = ["v.x", "v.y", "v.z", "v.w"][..n].join(" + ");
        let _ = writeln!(wgsl, "fn {PREFIX}dot_{n}(a:{f}, b:{f})->f32 {{ let v = {PREFIX}mul_{n}(a,b); return {sum}; }}
fn {PREFIX}length_{n}(a:{f})->f32 {{ return sqrt({PREFIX}dot_{n}(a,a)); }}
fn {PREFIX}distance_{n}(a:{f}, b:{f})->f32 {{ return {PREFIX}length_{n}(a-b); }}
fn {PREFIX}normalize_{n}(a:{f})->{f} {{ return a / {PREFIX}length_{n}(a); }}");
    }
    wgsl.push_str("fn loft_motion_round_cross_3(a:vec3f, b:vec3f)->vec3f {
    return loft_motion_round_mul_3(a.yzx,b.zxy) - loft_motion_round_mul_3(a.zxy,b.yzx);
}\n");
    wgsl
}

fn rewrite(f: &mut Function, info: &FunctionInfo, types: &naga::UniqueArena<naga::Type>, symbols: &BTreeMap<String, Handle<Function>>) {
    let lanes = |h: Handle<E>| match info[h].ty.inner_with(types) {
        T::Scalar(Scalar::F32) => 1,
        T::Vector { size, scalar: Scalar::F32 } => *size as usize,
        _ => 0,
    };
    let mut calls = Calls::new();
    for (h, e) in f.expressions.iter() {
        let (name, args) = match *e {
            E::Binary { op: naga::BinaryOperator::Multiply, left, right } => {
                let (a, b) = (lanes(left), lanes(right));
                let form = match (a, b) {
                    (0, _) | (_, 0) => continue,
                    (a, b) if a == b => "mul",
                    (1, _) => "scale_left",
                    (_, 1) => "scale",
                    _ => continue,
                };
                (format!("{form}_{}", a.max(b)), vec![left,right])
            }
            E::Math { fun, arg, arg1, arg2, .. } if lanes(arg) != 0 => {
                let n = lanes(arg);
                let form = match fun {
                    M::Dot => "dot", M::Length if n > 1 => "length", M::Distance if n > 1 => "distance",
                    M::Normalize => "normalize", M::Cross => "cross", M::Mix => "mix",
                    M::SmoothStep if n == 1 => "smooth", _ => continue,
                };
                let name = if fun == M::Mix { format!("{form}_{n}_{}", lanes(arg2.unwrap())) } else { format!("{form}_{n}") };
                (name, [Some(arg), arg1, arg2].into_iter().flatten().collect())
            }
            _ => continue,
        };
        calls.insert(h, (symbols[&format!("{PREFIX}{name}")], args));
    }
    for (&h, (function, _)) in &calls { f.expressions[h] = E::CallResult(*function); }
    split_emits(&mut f.body, &calls);
}

fn split_emits(body: &mut Block, calls: &Calls) {
    let mut out = Block::new();
    for (mut stmt, span) in std::mem::take(body).span_into_iter() {
        match &mut stmt {
            S::Emit(range) => {
                let mut part = None;
                let emit = |out: &mut Block, (a,b)| out.push(S::Emit(naga::Range::new_from_bounds(a,b)), span);
                for h in range.clone() {
                    if let Some((function, args)) = calls.get(&h) {
                        if let Some(part) = part.take() { emit(&mut out, part); }
                        out.push(S::Call { function: *function, arguments: args.clone(), result: Some(h) }, span);
                    } else {
                        part = Some((part.map_or(h, |(first, _)| first), h));
                    }
                }
                if let Some(part) = part { emit(&mut out, part); }
                continue;
            }
            S::Block(block) => split_emits(block, calls),
            S::If { accept, reject, .. } => { split_emits(accept, calls); split_emits(reject, calls); }
            S::Switch { cases, .. } => for case in cases { split_emits(&mut case.body, calls); },
            S::Loop { body, continuing, .. } => { split_emits(body, calls); split_emits(continuing, calls); }
            _ => {}
        }
        out.push(stmt, span);
    }
    *body = out;
}

pub(super) fn lower(source: &str, span: Span) -> Res<String> {
    let error = |msg: String| Diag::new(Code::E701, span, msg, "correct the fixed-tick shader");
    let validate = |m: &naga::Module| Validator::new(ValidationFlags::all(), Capabilities::default()).validate(m);
    let source = helpers() + source;
    let mut ir = naga::front::wgsl::parse_str(&source).map_err(|e| error(e.emit_to_string(&source)))?;
    let info = validate(&ir).map_err(|e| error(e.emit_to_string(&source)))?;
    let symbols = ir.functions.iter().filter_map(|(h, f)| {
        let name = f.name.as_ref()?.starts_with(PREFIX).then(|| f.name.clone().unwrap())?;
        Some((name, h))
    }).collect::<BTreeMap<_, _>>();
    for (h, f) in ir.functions.iter_mut() {
        if !f.name.as_ref().is_some_and(|name| symbols.contains_key(name)) {
            rewrite(f, &info[h], &ir.types, &symbols);
        }
    }
    for (i, ep) in ir.entry_points.iter_mut().enumerate() {
        rewrite(&mut ep.function, info.get_entry_point(i), &ir.types, &symbols);
    }
    let info = validate(&ir).map_err(|e| error(format!("{e:?}")))?;
    naga::back::wgsl::write_string(&ir, &info, naga::back::wgsl::WriterFlags::empty()).map_err(|e| error(e.to_string()))
}
