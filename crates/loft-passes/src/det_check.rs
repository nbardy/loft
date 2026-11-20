//! The authored raw-code contract at the typed shader boundary.
use crate::{
    Prog,
    emit::{MotionMath, References, motion_math},
    provenance::{self, Map},
};
use loft_ir::{Code, Diag, Res, Sources, Span};
use loft_parse::ast::{FnDecl, Ident};
use naga::{
    BinaryOperator as Op, Block, Expression as E, Function, Handle, Literal, LocalVariable,
    MathFunction as Math, Module, Statement as S, TypeInner as T,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type Helpers<'a> = BTreeMap<String, &'a FnDecl>;

pub(crate) fn source<'a>(
    p: &Prog<'a>,
    roots: impl IntoIterator<Item = &'a FnDecl>,
    constants: &BTreeSet<String>,
) -> Res<Vec<&'a Ident>> {
    let mut refs = References::default();
    for n in constants {
        refs.expr(&p.consts[n.as_str()].value, &[]);
    }
    for f in roots {
        if f.refuses_unverified_raw() {
            return Err(unmarked(f));
        }
        refs.calls.extend(References::function(f).calls);
    }
    for call in &refs.calls {
        if !p.fns.contains_key(call.name.as_str())
            && matches!(motion_math(&call.name), MotionMath::Unavailable)
        {
            return Err(Diag::new(
                Code::E504,
                call.span,
                "no deterministic motion implementation for this operation",
                "rewrite it using deterministic operations",
            ));
        }
    }
    Ok(refs.calls)
}

fn unmarked(f: &FnDecl) -> Diag {
    Diag::new(
        Code::E504,
        f.name.span,
        "motion reaches unverified raw WGSL",
        "use wgsl det (...) after verifying the block is deterministic",
    )
}

fn forbid(span: Span, reason: impl Into<String>) -> Diag {
    Diag::new(
        Code::E504,
        span,
        reason,
        "use bounded, local deterministic operations",
    )
}

/// A raw body's Naga byte offsets are relative to the same text in Loft's
/// provenance marker. Retain that marker when replacing fixed math calls.
struct Authored<'a> {
    decl: &'a FnDecl,
    body: Option<(usize, &'a str)>,
}

impl Authored<'_> {
    fn span(&self, map: &Map, src: &Sources, site: naga::Span) -> Span {
        let anchor = self.decl.name.span;
        if let Some((start, raw)) = self.body
            && let Some(range) = site.to_range()
            && let Some(offset) = range.start.checked_sub(start)
            && offset < raw.len() - 2
            && let Some((_, original)) = src.files.get(anchor.file as usize)
            && let Some(open) = original[anchor.lo as usize..].find(raw)
        {
            let lo = anchor.lo + (open + 1 + offset) as u32;
            return Span {
                file: anchor.file,
                lo,
                hi: lo + range.len() as u32,
            };
        }
        map.origin(site).unwrap_or(anchor)
    }
}

pub(crate) fn verify(
    module: &Module,
    map: &Map,
    src: &Sources,
    helpers: &Helpers<'_>,
    marked_shader: &mut String,
) -> Res<bool> {
    if !helpers.values().any(|f| f.raw.is_some()) {
        return Ok(false);
    }

    let mut pending = Vec::new();
    for entry in &module.entry_points {
        walk(&entry.function.body, &mut |s, _, _| {
            if let S::Call { function, .. } = s {
                pending.push((*function, false));
            }
            Ok(())
        })?;
    }
    let mut seen = BTreeSet::new();
    let mut changed = false;
    while let Some((id, from_raw)) = pending.pop() {
        let f = &module.functions[id];
        let name = f.name.as_deref().expect("named WGSL function");
        let decl = helpers.get(name).copied();
        let raw = decl.and_then(|d| d.raw.as_ref());
        let restricted = from_raw || raw.is_some();
        if !seen.insert((id, restricted)) {
            continue;
        }
        let authored = decl.map(|decl| Authored {
            decl,
            body: raw.map(|raw| {
                let start = module.functions.get_span(id).to_range().unwrap().start;
                let open = start
                    + map.shader[start..]
                        .find("{\n")
                        .expect("emitted function header");
                (open + 2, raw.body.as_str())
            }),
        });
        let Some(author) = authored.filter(|_| restricted) else {
            walk(&f.body, &mut |s, _, _| {
                if let S::Call { function, .. } = s {
                    pending.push((*function, false));
                }
                Ok(())
            })?;
            continue;
        };
        if author.decl.refuses_unverified_raw() {
            return Err(unmarked(author.decl));
        }
        let at = |site| author.span(map, src, site);
        // No pointer crosses an authored call, so callees cannot mutate a
        // caller's proven loop counter through an alias.
        for ty in f
            .arguments
            .iter()
            .map(|arg| arg.ty)
            .chain(f.result.iter().map(|ret| ret.ty))
        {
            let ty = &module.types[ty].inner;
            if matches!(ty, T::Pointer { .. } | T::ValuePointer { .. }) {
                return Err(forbid(
                    author.decl.name.span,
                    "raw motion cannot pass or return pointers",
                ));
            }
            if contains_f16(module, ty) {
                return Err(forbid(author.decl.name.span, "raw motion cannot use f16"));
            }
        }
        let function = Body { module, f };
        function.block(
            &f.body,
            &mut |callee, site| {
                let name = module.functions[callee].name.as_deref().unwrap();
                if helpers.contains_key(name) {
                    pending.push((callee, true));
                } else if !name
                    .strip_prefix("det_")
                    .is_some_and(|n| matches!(motion_math(n), MotionMath::Fixed))
                {
                    return Err(forbid(
                        at(site),
                        "raw motion cannot call generated shader code",
                    ));
                }
                Ok(())
            },
            &at,
        )?;

        let mut fixed = BTreeSet::new();
        let ctx = naga::proc::ResolveContext::with_locals(module, &f.local_variables, &f.arguments);
        let mut types = naga::front::Typifier::new();
        for (h, e) in f.expressions.iter() {
            let site = f.expressions.get_span(h);
            let ty = types
                .grow(h, &f.expressions, &ctx)
                .map(|()| types.get(h, &module.types))
                .map_err(|e| {
                    forbid(
                        at(site),
                        format!("cannot resolve raw motion expression: {e}"),
                    )
                })?;
            if contains_f16(module, ty) {
                return Err(forbid(at(site), "raw motion cannot use f16"));
            }
            if let Some(builtin) = expression(e).map_err(|reason| forbid(at(site), reason))? {
                let Some((start, raw)) = author.body else {
                    return Err(forbid(
                        at(site),
                        "unwrapped transcendental in a raw motion dependency",
                    ));
                };
                if *ty != T::Scalar(naga::Scalar::F32) {
                    return Err(forbid(
                        at(site),
                        "raw motion transcendental requires scalar f32",
                    ));
                }
                let range = site.to_range().expect("authored builtin span");
                if !map.shader[range.start..].starts_with(builtin) {
                    return Err(forbid(
                        at(site),
                        "cannot locate deterministic transcendental call",
                    ));
                }
                let offset = range
                    .start
                    .checked_sub(start)
                    .filter(|&offset| offset < raw.len() - 2)
                    .ok_or_else(|| {
                        forbid(at(site), "transcendental call is outside its raw body")
                    })?;
                fixed.insert(offset);
            }
        }
        for (_, local) in f.local_variables.iter() {
            if contains_f16(module, &module.types[local.ty].inner) {
                return Err(forbid(author.decl.name.span, "raw motion cannot use f16"));
            }
        }
        if !fixed.is_empty() {
            let (_, raw) = author.body.expect("fixed calls belong to a raw body");
            let original = &raw[1..raw.len() - 1];
            let mut body = original.to_owned();
            for offset in fixed.into_iter().rev() {
                body.insert_str(offset, "det_");
            }
            let span = author.decl.name.span;
            let marker = provenance::raw(span, original);
            let start = marked_shader
                .find(&marker)
                .expect("emitted raw provenance marker");
            marked_shader.replace_range(start..start + marker.len(), &provenance::raw(span, &body));
            changed = true;
        }
    }
    Ok(changed)
}

fn contains_f16(module: &Module, ty: &T) -> bool {
    match ty {
        T::Scalar(s)
        | T::Vector { scalar: s, .. }
        | T::Matrix { scalar: s, .. }
        | T::ValuePointer { scalar: s, .. }
        | T::Atomic(s) => s.kind == naga::ScalarKind::Float && s.width == 2,
        T::Array { base, .. } | T::Pointer { base, .. } => {
            contains_f16(module, &module.types[*base].inner)
        }
        T::Struct { members, .. } => members
            .iter()
            .any(|m| contains_f16(module, &module.types[m.ty].inner)),
        T::Image { .. }
        | T::Sampler { .. }
        | T::AccelerationStructure { .. }
        | T::RayQuery { .. }
        | T::BindingArray { .. }
        | T::CooperativeMatrix { .. } => false,
    }
}

fn expression(e: &E) -> Result<Option<&'static str>, String> {
    match e {
        E::Math { fun, .. } => math(*fun),
        E::Literal(_)
        | E::Constant(_)
        | E::ZeroValue(_)
        | E::Compose { .. }
        | E::Access { .. }
        | E::AccessIndex { .. }
        | E::Splat { .. }
        | E::Swizzle { .. }
        | E::FunctionArgument(_)
        | E::LocalVariable(_)
        | E::Load { .. }
        | E::Unary { .. }
        | E::Binary { .. }
        | E::Select { .. }
        | E::Relational { .. }
        | E::As { .. }
        | E::CallResult(_) => Ok(None),
        E::Override(_)
        | E::GlobalVariable(_)
        | E::ArrayLength(_)
        | E::ImageSample { .. }
        | E::ImageLoad { .. }
        | E::ImageQuery { .. }
        | E::Derivative { .. }
        | E::AtomicResult { .. }
        | E::WorkGroupUniformLoadResult { .. }
        | E::RayQueryVertexPositions { .. }
        | E::RayQueryProceedResult
        | E::RayQueryGetIntersection { .. }
        | E::SubgroupBallotResult
        | E::SubgroupOperationResult { .. }
        | E::CooperativeLoad { .. }
        | E::CooperativeMultiplyAdd { .. } => {
            Err("raw motion cannot access shader resources or invocation effects".into())
        }
    }
}

fn math(fun: Math) -> Result<Option<&'static str>, String> {
    match fun {
        Math::Sin => Ok(Some("sin")),
        Math::Cos => Ok(Some("cos")),
        Math::Atan2 => Ok(Some("atan2")),
        Math::Acos => Ok(Some("acos")),
        Math::Exp => Ok(Some("exp")),
        Math::Log => Ok(Some("log")),
        Math::Abs
        | Math::Min
        | Math::Max
        | Math::Clamp
        | Math::Sign
        | Math::Floor
        | Math::Ceil
        | Math::Trunc
        | Math::Fma
        | Math::Sqrt
        | Math::CountTrailingZeros
        | Math::CountLeadingZeros
        | Math::CountOneBits
        | Math::ReverseBits
        | Math::ExtractBits
        | Math::InsertBits
        | Math::FirstTrailingBit
        | Math::FirstLeadingBit => Ok(None),
        Math::Saturate
        | Math::Cosh
        | Math::Sinh
        | Math::Tan
        | Math::Tanh
        | Math::Asin
        | Math::Atan
        | Math::Asinh
        | Math::Acosh
        | Math::Atanh
        | Math::Radians
        | Math::Degrees
        | Math::Round
        | Math::Fract
        | Math::Modf
        | Math::Frexp
        | Math::Ldexp
        | Math::Exp2
        | Math::Log2
        | Math::Pow
        | Math::Dot
        | Math::Dot4I8Packed
        | Math::Dot4U8Packed
        | Math::Outer
        | Math::Cross
        | Math::Distance
        | Math::Length
        | Math::Normalize
        | Math::FaceForward
        | Math::Reflect
        | Math::Refract
        | Math::Mix
        | Math::Step
        | Math::SmoothStep
        | Math::InverseSqrt
        | Math::Inverse
        | Math::Transpose
        | Math::Determinant
        | Math::QuantizeToF16
        | Math::Pack4x8snorm
        | Math::Pack4x8unorm
        | Math::Pack2x16snorm
        | Math::Pack2x16unorm
        | Math::Pack2x16float
        | Math::Pack4xI8
        | Math::Pack4xU8
        | Math::Pack4xI8Clamp
        | Math::Pack4xU8Clamp
        | Math::Unpack4x8snorm
        | Math::Unpack4x8unorm
        | Math::Unpack2x16snorm
        | Math::Unpack2x16unorm
        | Math::Unpack2x16float
        | Math::Unpack4xI8
        | Math::Unpack4xU8 => Err(format!(
            "raw motion has no deterministic implementation for {fun:?}"
        )),
    }
}

fn walk(block: &Block, visit: &mut impl FnMut(&S, naga::Span, Option<&S>) -> Res<()>) -> Res<()> {
    let mut previous = None;
    for (s, span) in block.span_iter() {
        visit(s, *span, previous)?;
        match s {
            S::Block(b) => walk(b, visit)?,
            S::If { accept, reject, .. }
            | S::Loop {
                body: accept,
                continuing: reject,
                ..
            } => {
                walk(accept, visit)?;
                walk(reject, visit)?;
            }
            S::Switch { cases, .. } => {
                for case in cases {
                    walk(&case.body, visit)?;
                }
            }
            S::Emit(_) => continue,
            S::Break
            | S::Continue
            | S::Return { .. }
            | S::Kill
            | S::ControlBarrier(_)
            | S::MemoryBarrier(_)
            | S::Store { .. }
            | S::ImageStore { .. }
            | S::Atomic { .. }
            | S::ImageAtomic { .. }
            | S::WorkGroupUniformLoad { .. }
            | S::Call { .. }
            | S::RayQuery { .. }
            | S::RayPipelineFunction(_)
            | S::SubgroupBallot { .. }
            | S::SubgroupGather { .. }
            | S::SubgroupCollectiveOperation { .. }
            | S::CooperativeStore { .. } => {}
        }
        previous = Some(s);
    }
    Ok(())
}

struct Body<'a> {
    module: &'a Module,
    f: &'a Function,
}

impl Body<'_> {
    fn block(
        &self,
        block: &Block,
        call: &mut impl FnMut(Handle<Function>, naga::Span) -> Res<()>,
        at: &impl Fn(naga::Span) -> Span,
    ) -> Res<()> {
        walk(block, &mut |s, site, previous| {
            match s {
                S::Loop {
                    body,
                    continuing,
                    break_if,
                } => {
                    if break_if.is_some() || self.bound(body, continuing, site, previous).is_none()
                    {
                        return Err(forbid(
                            at(site),
                            "raw motion loop has no proven fixed bound",
                        ));
                    }
                }
                S::Store { pointer, .. } => {
                    if self.local(*pointer).is_none() {
                        return Err(forbid(
                            at(site),
                            "raw motion can only write function-local state",
                        ));
                    }
                }
                S::Call { function, .. } => call(*function, site)?,
                S::Block(_)
                | S::If { .. }
                | S::Switch { .. }
                | S::Emit(_)
                | S::Break
                | S::Continue
                | S::Return { .. } => {}
                S::Kill
                | S::ControlBarrier(_)
                | S::MemoryBarrier(_)
                | S::ImageStore { .. }
                | S::Atomic { .. }
                | S::ImageAtomic { .. }
                | S::WorkGroupUniformLoad { .. }
                | S::RayQuery { .. }
                | S::RayPipelineFunction(_)
                | S::SubgroupBallot { .. }
                | S::SubgroupGather { .. }
                | S::SubgroupCollectiveOperation { .. }
                | S::CooperativeStore { .. } => {
                    return Err(forbid(at(site), "unsupported raw motion effect"));
                }
            }
            Ok(())
        })
    }

    fn local(&self, mut ptr: Handle<E>) -> Option<Handle<LocalVariable>> {
        loop {
            match self.f.expressions[ptr] {
                E::LocalVariable(local) => return Some(local),
                E::Access { base, .. } | E::AccessIndex { base, .. } => ptr = base,
                _ => return None,
            }
        }
    }

    fn fresh_load<'b>(
        &self,
        e: Handle<E>,
        mut before: impl Iterator<Item = &'b S>,
    ) -> Option<Handle<E>> {
        // A load defined before the loop could be a let snapshot of the
        // counter. Only loads re-emitted at the guard/update see each step.
        match self.f.expressions[e] {
            E::Load { pointer }
                if before.any(
                    |s| matches!(s, S::Emit(r) if r.index_range().contains(&(e.index() as u32))),
                ) =>
            {
                Some(pointer)
            }
            _ => None,
        }
    }

    fn integer(&self, e: Handle<E>) -> Option<i64> {
        let mut expr = &self.f.expressions[e];
        loop {
            expr = match *expr {
                E::Literal(Literal::U32(v)) => return Some(v.into()),
                E::Literal(Literal::I32(v)) => return Some(v.into()),
                E::Constant(c) => &self.module.global_expressions[self.module.constants[c].init],
                _ => return None,
            };
        }
    }

    fn bound(
        &self,
        body: &Block,
        continuing: &Block,
        site: naga::Span,
        previous: Option<&S>,
    ) -> Option<i64> {
        // Naga lowers the for condition into the first if/break and the
        // increment into a separate block reached even by continue.
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
        let E::Binary { op, left, right } = self.f.expressions[*condition] else {
            return None;
        };
        let guard = body.iter().take_while(|s| matches!(s, S::Emit(_)));
        let pointer = self.fresh_load(left, guard)?;
        let E::LocalVariable(counter) = self.f.expressions[pointer] else {
            return None;
        };
        let local = &self.f.local_variables[counter];
        let (min, max) = match self.module.types[local.ty].inner {
            T::Scalar(naga::Scalar::I32) => (i64::from(i32::MIN), i64::from(i32::MAX)),
            T::Scalar(naga::Scalar::U32) => (0, i64::from(u32::MAX)),
            _ => return None,
        };
        let stop = self.integer(right)?;
        let mut updates = continuing.iter().filter(|s| !matches!(s, S::Emit(_)));
        let S::Store {
            pointer: target,
            value,
        } = updates.next()?
        else {
            return None;
        };
        if self.local(*target) != Some(counter) || updates.next().is_some() {
            return None;
        }
        let E::Binary {
            op: update,
            left,
            right,
        } = self.f.expressions[*value]
        else {
            return None;
        };
        let update_exprs = continuing.iter().take_while(|s| matches!(s, S::Emit(_)));
        if self
            .fresh_load(left, update_exprs)
            .and_then(|p| self.local(p))
            != Some(counter)
        {
            return None;
        }
        let step = match update {
            Op::Add => self.integer(right)?,
            Op::Subtract => -self.integer(right)?,
            _ => return None,
        };
        let start = match previous {
            Some(S::Store { pointer, value }) if self.local(*pointer) == Some(counter) => {
                self.integer(*value)?
            }
            _ => {
                // An initializer hoisted by Naga belongs to this loop only
                // when its declaration is in the for header.
                let decl = self.f.local_variables.get_span(counter).to_range()?;
                let site = site.to_range()?;
                if decl.start < site.start || decl.end > site.end {
                    return None;
                }
                // WGSL zero-initializes function variables without an initializer.
                local.init.map_or(Some(0), |init| self.integer(init))?
            }
        };
        let distance = match op {
            Op::Less if step > 0 => stop - start,
            Op::LessEqual if step > 0 => stop - start + 1,
            Op::Greater if step < 0 => start - stop,
            Op::GreaterEqual if step < 0 => start - stop + 1,
            _ => return None,
        };
        let trips = (distance.max(0) + step.abs() - 1) / step.abs();
        let last = start + trips * step;
        let mut body_writes_counter = false;
        walk(body, &mut |s, _, _| {
            if let S::Store { pointer, .. } = s
                && self.local(*pointer) == Some(counter)
            {
                body_writes_counter = true;
            }
            Ok(())
        })
        .expect("collecting loop stores cannot fail");
        ((min..=max).contains(&last) && !body_writes_counter).then_some(trips)
    }
}

/// Discover declarations to make available to Naga. A spelling is only a
/// dependency hint. Comments and local identifiers are not call targets.
pub(super) fn called_symbols(mut text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut previous = None;
    while let Some(first) = text.chars().next() {
        if let Some(comment) = text.strip_prefix("//") {
            text = comment.split_once('\n').map_or("", |(_, rest)| rest);
        } else if let Some(mut comment) = text.strip_prefix("/*") {
            let mut depth = 1;
            while depth != 0 && !comment.is_empty() {
                if let Some(rest) = comment.strip_prefix("/*") {
                    depth += 1;
                    comment = rest;
                } else if let Some(rest) = comment.strip_prefix("*/") {
                    depth -= 1;
                    comment = rest;
                } else {
                    comment = &comment[comment.chars().next().unwrap().len_utf8()..];
                }
            }
            text = comment;
        } else if first.is_alphanumeric() || first == '_' {
            let end = text
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(text.len());
            previous = Some(&text[..end]);
            text = &text[end..];
        } else {
            if first == '('
                && let Some(id) = previous
            {
                result.push(id);
            }
            if !first.is_whitespace() {
                previous = None;
            }
            text = &text[first.len_utf8()..];
        }
    }
    result
}
