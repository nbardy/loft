//! The fixed-tick entry is lowered as a typed call graph before it is serialized.
//! The CPU runner interprets that same WGSL, including the integer primitives.
use crate::provenance::Map;
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::{ExprKind, FnDecl};
use naga::{
    BinaryOperator as B, Block, Expression as E, Function, Handle, MathFunction as M,
    ScalarKind as K, Statement as S, TypeInner as T, UnaryOperator as U,
    valid::{Capabilities, FunctionInfo, ValidationFlags, Validator},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

const SCALAR: &str = include_str!("det_binary32.wgsl");
const LANES: [char; 4] = ['x', 'y', 'z', 'w'];

pub(super) fn refusal(span: Span, op: impl std::fmt::Display) -> Diag {
    Diag::new(
        Code::E504,
        span,
        format!("unverified motion operation: {op}"),
        "use bounded, row-local deterministic operations",
    )
}

pub(crate) fn source_function(f: &FnDecl) -> Res<()> {
    if f.refuses_unverified_raw() {
        return Err(refusal(f.name.span, "raw WGSL without det"));
    }
    // Loft binders are immutable. Literal intervals prove the trip count,
    // including loops nested inside expression arms.
    for (lo, hi, span) in super::References::function(f).loops {
        let literal = |e: &loft_parse::ast::Expr| match e.kind {
            ExprKind::Num { value, .. } => (0.0..=65536.0).contains(&value),
            _ => false,
        };
        if !literal(lo) || !literal(hi) {
            return Err(refusal(span, "for loop without a proven fixed bound"));
        }
    }
    Ok(())
}

struct Op {
    name: &'static str,
    args: Vec<Handle<E>>,
    shapes: Vec<usize>,
}
impl Op {
    fn symbol(&self) -> String {
        symbol(self.name, &self.shapes)
    }
}
fn symbol(name: &str, shapes: &[usize]) -> String {
    if shapes.iter().all(|&n| n == 1) {
        return format!("d_{name}");
    }
    let dims = shapes.iter().map(usize::to_string).collect::<String>();
    format!("d_{name}_{dims}")
}

fn wrappers() -> String {
    let mut out = String::new();
    let mut component = |op: &str, shapes: &[usize], input: &[&str], ret: &str| {
        let name = symbol(op, shapes);
        let width = *shapes.iter().max().unwrap();
        let ty = |n, scalar: &str| {
            if n == 1 {
                scalar.to_owned()
            } else {
                format!("vec{n}<{scalar}>")
            }
        };
        let params = shapes
            .iter()
            .zip(input)
            .enumerate()
            .map(|(i, (&n, t))| format!("a{i}: {}", ty(n, t)))
            .collect::<Vec<_>>()
            .join(", ");
        let values = LANES[..width]
            .iter()
            .map(|lane| {
                let args = shapes
                    .iter()
                    .enumerate()
                    .map(|(i, &n)| {
                        if n == 1 {
                            format!("a{i}")
                        } else {
                            format!("a{i}.{lane}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("d_{op}({args})")
            })
            .collect::<Vec<_>>()
            .join(", ");
        let result = ty(width, ret);
        writeln!(
            out,
            "fn {name}({params}) -> {result} {{ return {result}({values}); }}"
        )
        .unwrap();
    };
    for n in 2..=4 {
        for op in ["neg", "sqrt", "abs", "floor", "sign", "invsqrt"] {
            component(op, &[n], &["f32"], "f32");
        }
        for op in ["eq", "ne", "lt", "le", "gt", "ge"] {
            component(op, &[n, n], &["f32"; 2], "bool");
        }
        for op in ["add", "sub", "mul", "div", "min", "max", "step"] {
            for shapes in [[n, n], [n, 1], [1, n]] {
                component(op, &shapes, &["f32"; 2], "f32");
            }
        }
        for op in ["fma", "clamp", "smoothstep", "mix"] {
            for shapes in [[n, n, n], [n, n, 1], [n, 1, 1], [1, 1, n]] {
                component(op, &shapes, &["f32"; 3], "f32");
            }
        }
        for (op, from, to) in [
            ("fu", "u32", "f32"),
            ("fi", "i32", "f32"),
            ("uf", "f32", "u32"),
            ("if", "f32", "i32"),
        ] {
            component(op, &[n], &[from], to);
        }
    }
    for n in 2..=4 {
        let products = LANES[..n]
            .iter()
            .map(|lane| format!("d_mul(a.{lane}, b.{lane})"))
            .reduce(|a, b| format!("d_add({a}, {b})"))
            .unwrap();
        writeln!(
            out,
            "
fn d_dot_{n}{n}(a: vec{n}f, b: vec{n}f) -> f32 {{ return {products}; }}
fn d_len_{n}(a: vec{n}f) -> f32 {{ return d_sqrt(d_dot_{n}{n}(a,a)); }}
fn d_dist_{n}{n}(a: vec{n}f, b: vec{n}f) -> f32 {{ return d_len_{n}(d_sub_{n}{n}(a,b)); }}
fn d_norm_{n}(a: vec{n}f) -> vec{n}f {{ return d_div_{n}1(a,d_len_{n}(a)); }}
"
        )
        .unwrap();
    }
    out.push_str(
        "
fn d_cross_33(a: vec3f, b: vec3f) -> vec3f {
    return vec3f(d_sub(d_mul(a.y,b.z),d_mul(a.z,b.y)),
        d_sub(d_mul(a.z,b.x),d_mul(a.x,b.z)),d_sub(d_mul(a.x,b.y),d_mul(a.y,b.x)));
}
fn d_len(a: f32) -> f32 { return d_abs(a); }
fn d_dist(a: f32, b: f32) -> f32 { return d_abs(d_sub(a,b)); }
fn d_invsqrt(a: f32) -> f32 { return d_div(d_float(0x3f800000u),d_sqrt(a)); }
",
    );
    out
}

fn lanes(t: &T) -> Option<usize> {
    match t {
        T::Scalar(_) => Some(1),
        T::Vector { size, .. } => Some(*size as usize),
        _ => None,
    }
}

fn scalar(t: &T) -> Option<naga::Scalar> {
    match t {
        T::Scalar(s)
        | T::Vector { scalar: s, .. }
        | T::Matrix { scalar: s, .. }
        | T::Atomic(s)
        | T::ValuePointer { scalar: s, .. } => Some(*s),
        _ => None,
    }
}

fn floating(t: &T) -> bool {
    scalar(t).is_some_and(|s| s.kind == K::Float)
}

fn binary(op: B) -> Option<&'static str> {
    Some(match op {
        B::Add => "add",
        B::Subtract => "sub",
        B::Multiply => "mul",
        B::Divide => "div",
        B::Equal => "eq",
        B::NotEqual => "ne",
        B::Less => "lt",
        B::LessEqual => "le",
        B::Greater => "gt",
        B::GreaterEqual => "ge",
        B::Modulo
        | B::And
        | B::ExclusiveOr
        | B::InclusiveOr
        | B::LogicalAnd
        | B::LogicalOr
        | B::ShiftLeft
        | B::ShiftRight => return None,
    })
}

#[allow(clippy::wildcard_enum_match_arm)] // Closed set. A new Naga intrinsic needs a lowering.
fn math(op: M) -> Option<&'static str> {
    Some(match op {
        M::Abs => "abs",
        M::Min => "min",
        M::Max => "max",
        M::Clamp => "clamp",
        M::Sign => "sign",
        M::Floor => "floor",
        M::Fma => "fma",
        M::Sqrt => "sqrt",
        M::InverseSqrt => "invsqrt",
        M::Dot => "dot",
        M::Cross => "cross",
        M::Length => "len",
        M::Distance => "dist",
        M::Normalize => "norm",
        M::Step => "step",
        M::SmoothStep => "smoothstep",
        M::Mix => "mix",
        _ => return None,
    })
}

#[derive(Clone, Copy)]
struct Verify<'a> {
    map: &'a Map,
    helpers: &'a BTreeMap<String, Handle<Function>>,
    raw: &'a BTreeMap<String, Span>,
    fallback: Span,
    primitives: &'a BTreeSet<Handle<Function>>,
    module: &'a naga::Module,
}

impl Verify<'_> {
    fn error(&self, at: naga::Span, op: impl std::fmt::Display) -> Diag {
        refusal(self.map.origin(at).unwrap_or(self.fallback), op)
    }

    #[allow(clippy::wildcard_enum_match_arm)] // Closed expression set.
    fn expression(
        &self,
        expr: &E,
        info: &FunctionInfo,
        h: Handle<E>,
        at: naga::Span,
        raw: bool,
        primitive: bool,
    ) -> Res<Option<Op>> {
        let ty = |h: Handle<E>| info[h].ty.inner_with(&self.module.types);
        if scalar(ty(h)).is_some_and(|s| s.width != 4 && s.kind != K::Bool) {
            return Err(self.error(at, "scalar width outside binary32/i32/u32"));
        }
        let rewrite = |name, args: Vec<_>| {
            if primitive {
                return Err(self.error(at, "native float operation inside strict helpers"));
            }
            let shapes = args
                .iter()
                .map(|&h| lanes(ty(h)))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| self.error(at, "matrix arithmetic"))?;
            Ok(Some(Op { name, args, shapes }))
        };
        match *expr {
            E::Binary { op, left, right } if floating(ty(left)) || floating(ty(right)) => rewrite(
                binary(op).ok_or_else(|| self.error(at, format!("{op:?}")))?,
                vec![left, right],
            ),
            E::Unary {
                op: U::Negate,
                expr,
            } if floating(ty(expr)) => rewrite("neg", vec![expr]),
            E::Math {
                fun,
                arg,
                arg1,
                arg2,
                arg3,
            } if floating(ty(arg)) => rewrite(
                math(fun).ok_or_else(|| self.error(at, format!("{fun:?}")))?,
                std::iter::once(arg)
                    .chain([arg1, arg2, arg3].into_iter().flatten())
                    .collect(),
            ),
            E::Math {
                fun: M::Abs | M::Min | M::Max | M::Clamp | M::CountLeadingZeros,
                ..
            } => Ok(None),
            E::As {
                expr,
                kind,
                convert: Some(4),
            } => match (scalar(ty(expr)).map(|s| s.kind), kind) {
                (Some(K::Uint), K::Float) => rewrite("fu", vec![expr]),
                (Some(K::Sint), K::Float) => rewrite("fi", vec![expr]),
                (Some(K::Float), K::Uint) => rewrite("uf", vec![expr]),
                (Some(K::Float), K::Sint) => rewrite("if", vec![expr]),
                _ => Ok(None),
            },
            E::GlobalVariable(_) if raw => Err(self.error(at, "raw access to motion globals")),
            E::GlobalVariable(v)
                if matches!(
                    self.module.global_variables[v].space,
                    naga::AddressSpace::WorkGroup | naga::AddressSpace::Handle
                ) =>
            {
                Err(self.error(at, "shared workgroup or texture access"))
            }
            E::Literal(_)
            | E::Constant(_)
            | E::ZeroValue(_)
            | E::Compose { .. }
            | E::Access { .. }
            | E::AccessIndex { .. }
            | E::Splat { .. }
            | E::Swizzle { .. }
            | E::FunctionArgument(_)
            | E::GlobalVariable(_)
            | E::LocalVariable(_)
            | E::Load { .. }
            | E::Unary { .. }
            | E::Binary { .. }
            | E::Select { .. }
            | E::As { convert: None, .. }
            | E::CallResult(_) => Ok(None),
            E::AtomicResult { .. } if !raw => Ok(None),
            _ => Err(self.error(at, format!("{expr:?}"))),
        }
    }

    #[allow(clippy::wildcard_enum_match_arm)] // Closed effect set of the CPU entry.
    fn body(
        &self,
        f: &Function,
        block: &Block,
        raw: bool,
        events: bool,
        calls: &mut BTreeSet<Handle<Function>>,
    ) -> Res<()> {
        let mut previous = None;
        for (s, &at) in block.span_iter() {
            match s {
                S::Emit(_) | S::Return { .. } | S::Break | S::Continue | S::Store { .. } => {}
                S::Block(b) => self.body(f, b, raw, events, calls)?,
                S::If { accept, reject, .. } => {
                    self.body(f, accept, raw, events, calls)?;
                    self.body(f, reject, raw, events, calls)?;
                }
                S::Loop {
                    body,
                    continuing,
                    break_if,
                } => {
                    if raw
                        && (break_if.is_some()
                            || self
                                .raw_for_bound(f, body, continuing, at, previous)
                                .is_none())
                    {
                        return Err(self.error(at, "raw loop without a verified trip count"));
                    }
                    self.body(f, body, raw, events, calls)?;
                    self.body(f, continuing, raw, events, calls)?;
                }
                S::Call { function, .. } => {
                    let target = self.module.functions[*function]
                        .name
                        .as_deref()
                        .unwrap_or("");
                    if raw && !self.primitives.contains(function) && !self.raw.contains_key(target)
                    {
                        return Err(
                            self.error(at, format!("raw call to {target} with unverified effects"))
                        );
                    }
                    calls.insert(*function);
                }
                S::Atomic {
                    fun: naga::AtomicFunction::Add,
                    ..
                } if events => {}
                _ => return Err(self.error(at, format!("{s:?}"))),
            }
            if !matches!(s, S::Emit(_)) {
                previous = Some(s);
            }
        }
        Ok(())
    }

    fn raw_for_bound(
        &self,
        f: &Function,
        body: &Block,
        continuing: &Block,
        site: naga::Span,
        previous: Option<&S>,
    ) -> Option<()> {
        let fresh_load = |h: Handle<E>, block: &Block| match f.expressions[h] {
            E::Load { pointer }
                if block.iter().take_while(|s| matches!(s, S::Emit(_))).any(
                    |s| matches!(s, S::Emit(r) if r.index_range().contains(&(h.index() as u32))),
                ) =>
            {
                Some(pointer)
            }
            _ => None,
        };
        let integer = |h: Handle<E>| -> Option<i64> {
            let literal = match f.expressions[h] {
                E::Constant(c) => &self.module.global_expressions[self.module.constants[c].init],
                ref e => e,
            };
            match *literal {
                E::Literal(naga::Literal::U32(n)) => Some(i64::from(n)),
                E::Literal(naga::Literal::I32(n)) => Some(i64::from(n)),
                _ => None,
            }
        };
        // A WGSL for-loop becomes a leading guard and a continuing update.
        // Require fresh counter loads, a fixed initializer, and no body writes.
        let S::If {
            condition,
            accept,
            reject,
        } = body.iter().find(|s| !matches!(s, S::Emit(_)))?
        else {
            return None;
        };
        if !accept.is_empty() || !matches!(&**reject, [S::Break]) {
            return None;
        }
        let E::Binary { op, left, right } = f.expressions[*condition] else {
            return None;
        };
        let (counter, stop, op) = if let Some(counter) = fresh_load(left, body) {
            (counter, integer(right)?, op)
        } else {
            let op = match op {
                B::Less => B::Greater,
                B::LessEqual => B::GreaterEqual,
                B::Greater => B::Less,
                B::GreaterEqual => B::LessEqual,
                _ => return None,
            };
            (fresh_load(right, body)?, integer(left)?, op)
        };
        let E::LocalVariable(local) = f.expressions[counter] else {
            return None;
        };
        let init = &f.local_variables[local];
        let start = match previous {
            Some(S::Store { pointer, value })
                if f.expressions[*pointer] == E::LocalVariable(local) =>
            {
                integer(*value)?
            }
            _ => {
                let decl = f.local_variables.get_span(local).to_range()?;
                let site = site.to_range()?;
                if decl.start < site.start || decl.end > site.end {
                    return None;
                }
                init.init.map_or(Some(0), integer)?
            }
        };
        let mut updates = continuing.iter().filter(|s| !matches!(s, S::Emit(_)));
        let S::Store { pointer, value } = updates.next()? else {
            return None;
        };
        if f.expressions[*pointer] != E::LocalVariable(local) || updates.next().is_some() {
            return None;
        }
        let E::Binary {
            op: update,
            left,
            right,
        } = f.expressions[*value]
        else {
            return None;
        };
        if f.expressions[fresh_load(left, continuing)?] != E::LocalVariable(local) {
            return None;
        }
        let step = match update {
            B::Add => integer(right)?,
            B::Subtract => -integer(right)?,
            _ => return None,
        };
        let distance = match op {
            B::Less if step > 0 => stop - start,
            B::LessEqual if step > 0 => stop - start + 1,
            B::Greater if step < 0 => start - stop,
            B::GreaterEqual if step < 0 => start - stop + 1,
            _ => return None,
        };
        let trips = (distance.max(0) + step.abs() - 1) / step.abs();
        let last = start + trips * step;
        let representable = match self.module.types[init.ty].inner {
            T::Scalar(naga::Scalar::I32) => i32::try_from(last).is_ok(),
            T::Scalar(naga::Scalar::U32) => u32::try_from(last).is_ok(),
            _ => false,
        };
        (representable && trips <= 65_536 && !writes_local(f, body, local)).then_some(())
    }

    fn function(
        &self,
        f: &Function,
        info: &FunctionInfo,
        primitive: bool,
        calls: &mut BTreeSet<Handle<Function>>,
    ) -> Res<Rewrites> {
        let name = f.name.as_deref().unwrap_or("");
        let raw = self.raw.contains_key(name);
        if raw
            && f.arguments.iter().any(|arg| {
                matches!(
                    self.module.types[arg.ty].inner,
                    T::Pointer { .. } | T::ValuePointer { .. }
                )
            })
        {
            return Err(refusal(self.fallback, "raw pointer parameter"));
        }
        self.body(f, &f.body, raw, name == "emit_event", calls)?;
        let mut rewrites = BTreeMap::new();
        for (h, e) in f.expressions.iter() {
            let at = f.expressions.get_span(h);
            if let Some(op) = self.expression(e, info, h, at, raw, primitive)? {
                let target = self.helpers.get(&op.symbol()).copied().ok_or_else(|| {
                    self.error(at, format!("{} with shape {:?}", op.name, op.shapes))
                })?;
                calls.insert(target);
                rewrites.insert(h, (target, op.args));
            }
        }
        Ok(rewrites)
    }
}

fn writes_local(f: &Function, block: &Block, local: Handle<naga::LocalVariable>) -> bool {
    block.iter().any(|statement| match statement {
        S::Store { pointer, .. } => f.expressions[*pointer] == E::LocalVariable(local),
        S::Block(body) => writes_local(f, body, local),
        S::If { accept, reject, .. } => {
            writes_local(f, accept, local) || writes_local(f, reject, local)
        }
        S::Loop {
            body, continuing, ..
        } => writes_local(f, body, local) || writes_local(f, continuing, local),
        _ => false,
    })
}

type Rewrites = BTreeMap<Handle<E>, (Handle<Function>, Vec<Handle<E>>)>;

fn rewrite_block(block: Block, rewrites: &Rewrites) -> Block {
    let mut result = Block::new();
    for (stmt, at) in block.span_into_iter() {
        let stmt = match stmt {
            S::Emit(range) => {
                let mut first = None;
                let mut last = None;
                for h in range {
                    if let Some((function, args)) = rewrites.get(&h) {
                        if let Some((a, b)) = first.take().zip(last.take()) {
                            result.push(S::Emit(naga::Range::new_from_bounds(a, b)), at);
                        }
                        result.push(
                            S::Call {
                                function: *function,
                                arguments: args.clone(),
                                result: Some(h),
                            },
                            at,
                        );
                    } else {
                        first.get_or_insert(h);
                        last = Some(h);
                    }
                }
                if let Some((a, b)) = first.zip(last) {
                    result.push(S::Emit(naga::Range::new_from_bounds(a, b)), at);
                }
                continue;
            }
            S::Block(b) => S::Block(rewrite_block(b, rewrites)),
            S::If {
                condition,
                accept,
                reject,
            } => S::If {
                condition,
                accept: rewrite_block(accept, rewrites),
                reject: rewrite_block(reject, rewrites),
            },
            S::Loop {
                body,
                continuing,
                break_if,
            } => S::Loop {
                body: rewrite_block(body, rewrites),
                continuing: rewrite_block(continuing, rewrites),
                break_if,
            },
            other => other,
        };
        result.push(stmt, at);
    }
    result
}

fn apply(f: &mut Function, rewrites: &Rewrites) {
    for (&h, (target, _)) in rewrites {
        f.expressions[h] = E::CallResult(*target);
    }
    f.body = rewrite_block(std::mem::take(&mut f.body), rewrites);
}

pub(super) fn lower(wgsl: String, raw: &BTreeMap<String, Span>, fallback: Span) -> Res<String> {
    let primitives = format!("{SCALAR}\n{}\n", wrappers());
    let prefix_end = primitives.len();
    let map = Map::new(primitives + &wgsl);
    let mut module = naga::front::wgsl::parse_str(&map.shader).map_err(|e| {
        let span = e
            .labels()
            .find_map(|(at, _)| map.origin(at))
            .unwrap_or(fallback);
        refusal(span, e.emit_to_string(&map.shader))
    })?;
    let validate = |m: &naga::Module| {
        Validator::new(ValidationFlags::all(), Capabilities::default())
            .validate(m)
            .map_err(|e| {
                refusal(
                    e.spans()
                        .find_map(|(at, _)| map.origin(*at))
                        .unwrap_or(fallback),
                    e.emit_to_string(&map.shader),
                )
            })
    };
    let info = validate(&module)?;
    let mut helpers = BTreeMap::new();
    for (h, f) in module.functions.iter() {
        let at = module.functions.get_span(h);
        if at.to_range().is_some_and(|r| r.start < prefix_end) {
            helpers.insert(f.name.clone().expect("named strict primitive"), h);
        }
    }
    let primitives = helpers.values().copied().collect();
    let verifier = Verify {
        map: &map,
        helpers: &helpers,
        raw,
        fallback,
        primitives: &primitives,
        module: &module,
    };
    let mut todo = BTreeSet::new();
    let entry = verifier.function(
        &module.entry_points[0].function,
        info.get_entry_point(0),
        false,
        &mut todo,
    )?;
    let mut reached = BTreeMap::new();
    while let Some(h) = todo.pop_first() {
        if reached.contains_key(&h) {
            continue;
        }
        let mut v = Verify {
            fallback: raw
                .get(module.functions[h].name.as_deref().unwrap_or(""))
                .copied()
                .unwrap_or(fallback),
            ..verifier
        };
        if let Some(span) = map.origin(module.functions.get_span(h)) {
            v.fallback = span;
        }
        let rewrites = v.function(
            &module.functions[h],
            &info[h],
            primitives.contains(&h),
            &mut todo,
        )?;
        reached.insert(h, rewrites);
    }
    for (h, rewrites) in reached {
        apply(&mut module.functions[h], &rewrites);
    }
    apply(&mut module.entry_points[0].function, &entry);
    let _ = validate(&module)?;
    naga::compact::compact(&mut module, naga::compact::KeepUnused::No);
    let verified = validate(&module)?;
    naga::back::wgsl::write_string(&module, &verified, naga::back::wgsl::WriterFlags::empty())
        .map_err(|e| refusal(fallback, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_raw_loop_bounds_and_rejects_native_float_ops() {
        let shader = |body: &str| {
            format!(
                r"
            @group(0) @binding(0) var<storage, read_write> data: array<f32>;
            fn raw(x: f32) -> f32 {{ {body} }}
            @compute @workgroup_size(1) fn advance() {{ data[0] = raw(data[0]); }}
        "
            )
        };
        let span = Span {
            file: 0,
            lo: 12,
            hi: 15,
        };
        let raw = BTreeMap::from([("raw".into(), span)]);
        for guard in ["i < 3u", "3u > i"] {
            let body =
                format!("var y = x; for (var i = 0u; {guard}; i++) {{ y = y * x + x; }} return y;");
            let lowered = lower(shader(&body), &raw, span).unwrap();
            assert!(lowered.contains("d_mul_bits") && lowered.contains("d_add_bits"));
        }
        for body in [
            "var y = x; for (var i = 0u; i < u32(x); i++) { y = y * x; } return y;",
            "for (var i = 0u; i < 3u; i++) { i = 0u; } return x;",
            "for (var i = 0u; i >= 0u; i--) {} return x;",
            "loop {} return x;",
            "return pow(x, x);",
            "return data[0];",
        ] {
            let error = lower(shader(body), &raw, Span::NONE).unwrap_err();
            assert_eq!(error.code, Code::E504, "{body}");
            assert_eq!(error.span, span, "{body}");
        }
    }
}
