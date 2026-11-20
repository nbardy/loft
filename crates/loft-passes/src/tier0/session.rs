//! Source admission, affected checking and conservative bounds for one dirty kind.
use super::TraceEdit;
use super::author::{Certificates, Operand, Residual, lower_trace, shader_queries, unsupported};
use crate::{
    Compiled, Eval, Prog, RowVal, emit, lip,
    program::{Draw, Instance, Program},
};
use loft_ir::{Code, Diag, Res, Sources, Span};
use loft_parse::{
    ast::*,
    cst::{Cst, Node, Token, Trivia},
};
use std::{
    ops::Range,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditClass {
    ConstantPool,
    StructuralTape,
    IncompatibleLayout,
}

pub struct PreparedEdit {
    pub tape: TraceEdit,
    pub shader: String,
    /// Replacement hidden columns and culling entries, in the kind's row order.
    pub bounds: Vec<Instance>,
    pub local_bounds: Vec<[[f32; 3]; 2]>,
    pub lips: Vec<f32>,
    pub parse_check: Duration,
    pub bounds_lowering: Duration,
}

impl PreparedEdit {
    pub fn class(&self, previous: &TraceEdit) -> EditClass {
        if self.tape.slots != previous.slots {
            EditClass::IncompatibleLayout
        } else if self.tape.words == previous.words {
            EditClass::ConstantPool
        } else {
            EditClass::StructuralTape
        }
    }
}

struct Site {
    helper: Option<String>,
    range: Range<usize>,
}

pub struct TraceSource {
    source: Sources,
    compiled: Compiled,
    kind: usize,
    file: u16,
    name: String,
    sites: Vec<Site>,
    certificates: Certificates,
}

fn reload(span: Span) -> Diag {
    Diag::new(
        Code::E405,
        span,
        "incompatible edit: source outside the dirty trace changed",
        "reload the scene to change its layout or other queries",
    )
}

impl TraceSource {
    pub fn new(
        source: Sources,
        compiled: Compiled,
        program: &Program,
        name: &str,
    ) -> Res<(Self, TraceEdit)> {
        let p = compiled.prog();
        let k = p
            .kinds
            .get(name)
            .ok_or_else(|| unsupported(Span::NONE, "unknown edited kind"))?;
        let trace = k
            .query(QueryKind::Trace)
            .ok_or_else(|| unsupported(k.name.span, "a drawn trace query is required"))?;
        let kind = program
            .kinds
            .iter()
            .position(|k| k.name == name && k.draw == Draw::Traced)
            .ok_or_else(|| unsupported(k.name.span, "edit mode requires a sphere-traced kind"))?;
        if program.path.is_some() || !program.bakes.is_empty() || !program.dispatches.is_empty() {
            return Err(unsupported(
                k.name.span,
                "the edit session requires a render-only kind without baked resources",
            ));
        }
        let file = trace.name.span.file;
        let text = &source.files[usize::from(file)].1;
        let cst = Cst::parse(file, text)?;
        let range = |f: &FnDecl| {
            body_range(&cst.roots, text, f.name.span).ok_or_else(|| reload(f.name.span))
        };
        let mut sites = vec![Site {
            helper: None,
            range: range(trace)?,
        }];
        let others: Vec<_> = p
            .kinds
            .values()
            .flat_map(|k| &k.queries)
            .map(|(_, f)| f)
            .filter(|f| !std::ptr::eq(*f, trace))
            .chain(p.fns.values().copied().filter(|f| f.comptime))
            .chain(p.checks.values().copied())
            .chain(p.laws.values().map(|law| &law.f))
            .collect();
        let (uneditable, _) = emit::reachable(&p, &others, &[light(&p)?]);
        let (helpers, _) = emit::reachable(&p, &[trace], &[]);
        for helper in helpers {
            if helper.name.span.file == file
                && !helper.comptime
                && helper.raw.is_none()
                && !uneditable
                    .iter()
                    .chain(&others)
                    .any(|&f| std::ptr::eq(f, helper))
            {
                sites.push(Site {
                    helper: Some(helper.name.name.clone()),
                    range: range(helper)?,
                });
            }
        }
        sites.sort_by_key(|s| s.range.start);
        let certificates = Certificates::new(&p)?;
        let mut tape = lower_trace(&p, &compiled.types, k, &certificates)?;
        tape.shader = render_shader(&compiled, &p, k, &tape)?;
        let name = name.to_owned();
        Ok((
            Self {
                source,
                compiled,
                kind,
                file,
                name,
                sites,
                certificates,
            },
            tape,
        ))
    }

    pub fn kind(&self) -> usize {
        self.kind
    }

    pub fn prepare(&self, edited: &crate::source::Graph, overrides: &[(String, f64)]) -> Res<PreparedEdit> {
        let start = Instant::now();
        let mut p = self.compiled.prog();
        let initial = p.kinds[self.name.as_str()];
        let original = initial.query(QueryKind::Trace).expect("retained trace");
        let file = usize::from(self.file);
        if edited.files.len() != self.source.files.len()
            || edited
                .files
                .iter()
                .zip(&self.source.files)
                .enumerate()
                .any(|(i, (new, old))| i != file && new != old)
        {
            return Err(reload(original.name.span));
        }
        let old_text = &self.source.files[file].1;
        let text = &edited.files[file].1;
        let mut next = self.compiled.types.len() as u32;
        let module = edited.edited_module(self.file, &mut next)?;
        if !module.diffs.is_empty() {
            return Err(unsupported(
                module.diffs[0].0.span,
                "an edited trace cannot introduce derivative lowering",
            ));
        }
        let cst = Cst::parse(self.file, text)?;
        let fresh = Prog::index(std::slice::from_ref(&module))?;
        let fresh_kind = fresh
            .kinds
            .get(self.name.as_str())
            .ok_or_else(|| reload(original.name.span))?;
        let trace = fresh_kind
            .query(QueryKind::Trace)
            .ok_or_else(|| reload(fresh_kind.name.span))?;
        let mut replacements = Vec::new();
        let (mut old_end, mut new_end) = (0, 0);
        for site in &self.sites {
            let f = match &site.helper {
                Some(name) => fresh
                    .fns
                    .get(name.as_str())
                    .copied()
                    .ok_or_else(|| reload(trace.name.span))?,
                None => trace,
            };
            let range =
                body_range(&cst.roots, text, f.name.span).ok_or_else(|| reload(f.name.span))?;
            if text.get(new_end..range.start) != old_text.get(old_end..site.range.start) {
                return Err(reload(f.name.span));
            }
            if let Some(name) = &site.helper
                && text[range.clone()] != old_text[site.range.clone()]
            {
                let old = p.fns[name.as_str()];
                // The unchanged signature includes the elaborated types of private helpers.
                let mut replacement = old.clone();
                replacement.body = f.body.clone();
                replacement.name.span = f.name.span;
                replacements.push(replacement);
            }
            old_end = site.range.end;
            new_end = range.end;
        }
        if text[new_end..] != old_text[old_end..] {
            return Err(reload(trace.name.span));
        }

        let mut kind = initial.clone();
        let retained = kind
            .queries
            .iter_mut()
            .find(|(q, _)| *q == QueryKind::Trace)
            .expect("retained trace");
        retained.1 = trace.clone();
        p.kinds.insert(self.name.as_str(), &kind);
        p.dimensions.retain(|span, _| {
            span.file != self.file
                || usize::try_from(span.lo)
                    .is_ok_and(|lo| !self.sites.iter().any(|s| s.range.contains(&lo)))
        });
        p.dimensions.extend(module.dimensions.iter().copied());
        for replacement in &replacements {
            p.fns.insert(replacement.name.name.as_str(), replacement);
        }
        // Raw islands are lifted by parsing. Keep their new spans so the
        // checked geometry's E211 is the same whether editing or loading.
        for (&name, &f) in &fresh.fns {
            if f.raw.is_some() && !p.fns.contains_key(name) {
                p.fns.insert(name, f);
            }
        }
        let trace = kind.query(QueryKind::Trace).expect("replacement trace");
        if let Some(raw) = emit::proof_dependencies(&p, trace)
            .into_iter()
            .find(|f| f.raw.is_some())
        {
            return Err(Diag::new(
                Code::E211,
                raw.name.span,
                "raw WGSL has no proven spatial Lipschitz bound",
                "write the trace term in Loft or move it to the material",
            ));
        }
        let mut types = self.compiled.types.clone();
        let mut checker = crate::check::Ck::new(&p, &mut types);
        for f in &replacements {
            checker.func(f, None)?;
        }
        checker.func(trace, Some(self.name.as_str()))?;
        let kinds = std::mem::replace(&mut p.kinds, [(self.name.as_str(), &kind)].into());
        crate::refine::check(&p, &types, overrides)?;
        p.kinds = kinds;
        let parse_check = start.elapsed();
        let mark = Instant::now();
        let tape = lower_trace(&p, &types, &kind, &self.certificates)?;
        let (bounds, local_bounds, lips) = self.bounds(&p, &types, &kind, &tape, overrides)?;
        let (light, luts) = render_inputs(&self.compiled, &p)?;
        let shader = device_shader(emit::kind_module(
            &p,
            &types,
            &kind,
            light,
            &luts,
            Draw::Traced,
            emit::Aov::Beauty,
        )?);
        Ok(PreparedEdit {
            tape,
            shader,
            bounds,
            local_bounds,
            lips,
            parse_check,
            bounds_lowering: mark.elapsed(),
        })
    }

    fn bounds(
        &self,
        p: &Prog<'_>,
        types: &[Ty],
        kind: &KindDecl,
        tape: &TraceEdit,
        overrides: &[(String, f64)],
    ) -> Res<(Vec<Instance>, Vec<[[f32; 3]; 2]>, Vec<f32>)> {
        let (table, _) = &self.compiled.drawn[self.kind];
        let rows = &self.compiled.tables[*table];
        let span = kind.name.span;
        let trace = kind.query(QueryKind::Trace).expect("checked trace");
        let mut evaluator = None;
        let fields = crate::geometry_fields(kind);
        let mut derived: Vec<(Vec<&RowVal>, lip::Derived)> = Vec::new();
        let mut instances = Vec::with_capacity(rows.len());
        let mut lips = Vec::with_capacity(rows.len());
        let mut local_bounds = Vec::with_capacity(rows.len());
        for (r, row) in rows.iter().enumerate() {
            let key = fields.iter().map(|&field| &row[field]).collect::<Vec<_>>();
            let primitive = primitive_bound(tape, row);
            let d = if let Some(d) = &primitive {
                d
            } else if let Some(i) = derived.iter().position(|(k, _)| *k == key) {
                &derived[i].1
            } else {
                let ev = evaluator.get_or_insert_with(|| {
                    let mut ev = Eval::new(p, types, self.compiled.tables.clone(), overrides);
                    ev.luts = &self.compiled.luts;
                    ev
                });
                let d = lip::derive(ev, trace, *table, r, span)?;
                derived.push((key, d));
                &derived.last().expect("new bound").1
            };
            let pose = match crate::pose_field(kind).map(|i| &row[i]) {
                Some(RowVal::X(x)) => *x,
                _ => crate::eval::Rigid::yaw([0.; 3], 0.),
            };
            let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
            for corner in 0..8 {
                let q = std::array::from_fn(|i| {
                    if corner >> i & 1 == 0 {
                        d.lo[i] as f32
                    } else {
                        d.hi[i] as f32
                    }
                });
                let q = crate::rot(&pose, q);
                for i in 0..3 {
                    lo[i] = lo[i].min(q[i] + pose.t[i]);
                    hi[i] = hi[i].max(q[i] + pose.t[i]);
                }
            }
            instances.push(Instance {
                lo,
                hi,
                kind: self.kind as u32,
                row: r as u32,
            });
            // Marching reads local coordinates. Only the culling table uses
            // the transformed AABB.
            local_bounds.push([d.lo, d.hi].map(|v| v.map(|x| x as f32)));
            lips.push(d.l as f32);
        }
        Ok((instances, local_bounds, lips))
    }
}

fn primitive_bound(tape: &TraceEdit, row: &[RowVal]) -> Option<lip::Derived> {
    let load = |op| match op {
        Operand::Zero => Some([0.; 4]),
        Operand::Constant(i) => Some(tape.constants[usize::from(i)]),
        Operand::Row(i) => match &row[tape.slots[usize::from(i)]] {
            RowVal::F(v) => Some([*v; 4]),
            RowVal::V(v) => Some(std::array::from_fn(|i| {
                v.get(i).copied().unwrap_or_default()
            })),
            _ => None,
        },
        Operand::Frame(_) => None,
    };
    let (c, half) = match tape.residual {
        Residual::Tape => return None,
        Residual::Sphere { center, radius } => (load(center)?, [load(radius)?[0]; 3]),
        Residual::Box { center, half } => {
            let [x, y, z, _] = load(half)?;
            (load(center)?, [x, y, z])
        }
    };
    if !half.iter().all(|r| (0.0..=1.25).contains(r) && *r != 0.0) {
        return None;
    }
    // Use the interval prover's leaf grid, including cells that touch a
    // zero exactly on their face. Small certified primitives have L=1.
    let round = |x: f64| ((x / lip::CELL).floor() + 1.0) * lip::CELL;
    let mut lo = std::array::from_fn(|i| -round(f64::from(half[i]) - f64::from(c[i])));
    let mut hi = std::array::from_fn(|i| round(f64::from(c[i]) + f64::from(half[i])));
    for (lo, hi) in lo.iter_mut().zip(&mut hi) {
        if lo.abs().max(hi.abs()) >= lip::REACH {
            return None;
        }
        let pad = lip::PAD + lip::PAD_REL * (*hi - *lo);
        *lo -= pad;
        *hi += pad;
    }
    Some(lip::Derived {
        lo,
        hi,
        l: 1.,
        evals: 0,
    })
}

fn light<'a>(p: &Prog<'a>) -> Res<&'a str> {
    let shade = p
        .schedule
        .and_then(|s| s.passes.iter().find(|pd| pd.verb.name == "shade_tiles"))
        .ok_or_else(|| unsupported(Span::NONE, "trace edits require a tiled shade pass"))?;
    crate::fn_name_arg(p, shade, "light")
}

fn render_inputs<'a>(compiled: &Compiled, p: &Prog<'a>) -> Res<(&'a str, Vec<emit::LutRef>)> {
    let mut offset = crate::gltf::words(&compiled.meshes).len().div_ceil(12) as u32;
    let luts = compiled
        .luts
        .iter()
        .map(|l| {
            let n = l.frames.len() as u32;
            let lut = emit::LutRef {
                name: l.name.clone(),
                offset,
                n,
                lo: l.lo,
                hi: l.hi,
            };
            offset += n;
            lut
        })
        .collect();
    Ok((light(p)?, luts))
}

pub(super) fn render_shader(
    compiled: &Compiled,
    p: &Prog<'_>,
    kind: &KindDecl,
    tape: &TraceEdit,
) -> Res<String> {
    let (light, luts) = render_inputs(compiled, p)?;
    let edit = shader_queries(p, kind, tape)?;
    Ok(device_shader(emit::kind_module_edit(
        p,
        &compiled.types,
        kind,
        light,
        &luts,
        Draw::Traced,
        emit::Aov::Beauty,
        &edit,
    )?))
}

fn device_shader(wgsl: String) -> String {
    let mut text = crate::quant::wgsl_prelude(&wgsl, loft_ir::GpuFeatures::empty());
    if wgsl.contains("loft_ggx_energy(") {
        let table = wgsl.contains("var<storage, read> luts:").then_some("luts");
        text.push_str(&crate::bsdf::energy_wgsl(table));
    }
    text.push_str(&wgsl);
    // These paths go straight to wgpu, bypassing compile's provenance map.
    crate::provenance::Map::new(text).shader
}

fn body_range(nodes: &[Node], text: &str, name: Span) -> Option<Range<usize>> {
    fn token(n: &Node) -> &Token {
        match n {
            Node::Leaf(t) => t,
            Node::Group { open, .. } => open,
        }
    }
    for (i, node) in nodes.iter().enumerate() {
        if let Node::Group { children, .. } = node {
            if let Some(range) = body_range(children, text, name) {
                return Some(range);
            }
            continue;
        }
        if token(node).span != name {
            continue;
        }
        let mut body = nodes[i + 1..]
            .iter()
            .skip_while(|n| !matches!(token(n).text(text), "=" | "{"));
        let head = body.next()?;
        let start = token(head).span.lo as usize;
        if let Node::Group { close, .. } = head {
            return Some(start..close.span.hi as usize);
        }
        let mut end = token(head).span.hi as usize;
        for n in body {
            let t = token(n);
            if t.text(text) == ";"
                || t.kind == loft_parse::cst::Kind::End
                || t.leading
                    .iter()
                    .any(|(trivia, _)| *trivia == Trivia::Newline)
            {
                break;
            }
            end = match n {
                Node::Leaf(t) => t.span.hi,
                Node::Group { close, .. } => close.span.hi,
            } as usize;
        }
        return Some(start..end);
    }
    None
}
