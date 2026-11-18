//! Everything the host refuses, decided before any GPU object exists (plan §5.1 step 6: a game keeps its old `Loft`
//! when a hot reload is refused). Ids in range, shapes fit for their roles, sizes within the device's limits, and every
//! module through Naga under the precision that will run.
use crate::{Stepping, entry_of, prelude, state_bytes};
use loft_ir::Error;
use loft_ir::bundle::{
    Access, Bundle, Cover, Find, Id, Init, Origin, Proxy, RasterPrim, Shape, Target, TexFormat, When,
};
use loft_ir::error::{LoadFault, SceneFault};
use loft_ir::identity::Precision;

fn bad(m: String) -> Error {
    Error::Load(LoadFault::Invalid(m.into()))
}

fn at<'a, T>(list: &'a [T], id: Id, what: &str) -> Result<&'a T, Error> {
    list.get(id as usize).ok_or_else(|| bad(format!("{what} {id} is out of range (have {})", list.len())))
}

/// A resource a role needs as rows (`rows`) or as a colour or depth texture (`depth`).
fn rows(b: &Bundle, r: Id, what: &str) -> Result<(), Error> {
    match at(&b.resources, r, what)?.shape {
        Shape::Rows { .. } => Ok(()),
        Shape::Texture { .. } | Shape::ViewSized { .. } | Shape::Frame => Err(bad(format!("{what} {r} is not rows"))),
    }
}
fn texture(b: &Bundle, r: Id, depth: bool, what: &str) -> Result<(), Error> {
    let format = match at(&b.resources, r, what)?.shape {
        Shape::Texture { format, .. } | Shape::ViewSized { format, .. } => format,
        Shape::Rows { .. } | Shape::Frame => return Err(bad(format!("{what} {r} is not a texture"))),
    };
    match (format == TexFormat::Depth32Float) == depth {
        true => Ok(()),
        false => Err(bad(format!("{what} {r} is {format:?}; it needs {}", if depth { "depth" } else { "colour" }))),
    }
}

/// The packs: every rows resource in exactly one, inside it and overlapping nothing; a stepped resource alone in a
/// written pack at offset 0 (the host swaps the whole buffer per step); a kernel's rows in a written pack; and every
/// stage's storage bindings within the device's limit (a kernel binds its writes once more at group 2).
fn packs(b: &Bundle, limits: &wgpu::Limits) -> Result<(), Error> {
    let kept = |r: usize| b.kernels.iter().find(|k| k.writes.contains(&(r as Id))).and_then(Stepping::of);
    let mut seen = vec![None; b.resources.len()];
    for (p, pack) in b.packs.iter().enumerate() {
        let size = pack.vec4s.checked_mul(16).filter(|&n| n > 0);
        let max = limits.max_storage_buffer_binding_size.min(limits.max_buffer_size);
        if size.is_none_or(|n| n > max) {
            return Err(bad(format!(
                "pack {p} is {} vec4s; this device binds 1..={} : fewer or smaller tables",
                pack.vec4s,
                max / 16
            )));
        }
        let mut spans = Vec::with_capacity(pack.holds.len());
        for &(r, offset) in pack.holds.iter() {
            let res = at(&b.resources, r, "pack resource")?;
            if !matches!(res.shape, Shape::Rows { .. }) {
                return Err(bad(format!("pack {p} holds resource {r}, which is not rows")));
            }
            if seen[r as usize].replace(p).is_some() {
                return Err(bad(format!("resource {r} is in two packs")));
            }
            let vec4s = state_bytes(&res.shape) / 16 * u64::from(kept(r as usize).map_or(1, |s| s.kept));
            let end = offset.checked_add(vec4s).filter(|&e| e <= pack.vec4s);
            let end = end.ok_or_else(|| bad(format!("resource {r} at vec4 {offset} overruns pack {p}")))?;
            spans.push((offset, end, r));
            if kept(r as usize).is_some() && (pack.holds.len() != 1 || offset != 0 || pack.access != Access::Write) {
                return Err(bad(format!("stepped resource {r} must be alone in a written pack, at offset 0")));
            }
        }
        spans.sort_unstable();
        if let Some(w) = spans.windows(2).find(|w| w[0].1 > w[1].0) {
            return Err(bad(format!("resources {} and {} overlap in pack {p}", w[0].2, w[1].2)));
        }
    }
    for (r, res) in b.resources.iter().enumerate() {
        if matches!(res.shape, Shape::Rows { .. }) && seen[r].is_none() {
            return Err(bad(format!("rows resource {r} is in no pack")));
        }
    }
    let stage = limits.max_storage_buffers_per_shader_stage as usize;
    for (k, kern) in b.kernels.iter().enumerate() {
        for &r in kern.writes.iter() {
            if let Some(p) = seen[r as usize].filter(|&p| b.packs[p].access != Access::Write) {
                return Err(bad(format!("kernel {k} writes resource {r} in pack {p}, which is read-only")));
            }
        }
        if b.packs.len() + kern.writes.len() > stage {
            return Err(bad(format!(
                "kernel {k} binds {} packs and {} writes; this device binds {stage} storage buffers per stage",
                b.packs.len(),
                kern.writes.len()
            )));
        }
    }
    if b.packs.len() > stage {
        return Err(bad(format!("{} packs; this device binds {stage} storage buffers per stage", b.packs.len())));
    }
    Ok(())
}

pub fn validate(b: &Bundle, precision: Precision, limits: &wgpu::Limits) -> Result<(), Error> {
    let entry = |(m, e): (Id, Id), what: &str| at(&b.modules, m, what).and(at(&b.names, e, "entry name")).map(|_| ());
    for (r, res) in b.resources.iter().enumerate() {
        let kept = b
            .kernels
            .iter()
            .find(|k| k.writes.contains(&(r as Id)))
            .map_or(1, |k| Stepping::of(k).map_or(1, |s| s.kept));
        let state = match res.shape {
            Shape::Rows { rows, lanes } if rows > 0 && lanes > 0 => state_bytes(&res.shape),
            Shape::Rows { .. } => return Err(bad(format!("resource {r} has no rows or lanes"))),
            Shape::Texture { extent, .. } => {
                let max = if extent[2] > 1 { limits.max_texture_dimension_3d } else { limits.max_texture_dimension_2d };
                if extent.iter().any(|&e| e == 0 || e > max) {
                    return Err(bad(format!("texture {r} is {extent:?}; each side must be 1..={max}")));
                }
                state_bytes(&res.shape)
            }
            Shape::ViewSized { .. } | Shape::Frame => 0,
        };
        let max = limits.max_storage_buffer_binding_size.min(limits.max_buffer_size);
        if state * u64::from(kept) > max {
            return Err(bad(format!(
                "resource {r} needs {} bytes; this device binds {max}: keep fewer states or rows",
                state * u64::from(kept)
            )));
        }
        match res.init {
            Init::Zero => {}
            Init::Bytes(s) => {
                let n = at(&b.sections, s, "section")?.len() as u64;
                if n != state || state == 0 {
                    return Err(bad(format!(
                        "resource {r} is filled by section {s} of {n} bytes; one state is {state}"
                    )));
                }
            }
            Init::By(k) => {
                let k = at(&b.kernels, k, "init kernel")?;
                if !matches!(k.when, When::Load) || !k.writes.contains(&(r as Id)) {
                    return Err(bad(format!("resource {r}'s init kernel must run at load and write it")));
                }
            }
        }
    }
    for (k, kern) in b.kernels.iter().enumerate() {
        for &r in kern.reads.iter() {
            at(&b.resources, r, "kernel read")?;
        }
        for &r in kern.writes.iter() {
            match at(&b.resources, r, "kernel write")?.shape {
                Shape::Rows { .. } => {}
                Shape::Texture { .. } | Shape::ViewSized { .. } => texture(b, r, false, "kernel write")?,
                Shape::Frame => return Err(bad(format!("kernel {k} writes the frame uniform, which the host owns"))),
            }
        }
        entry(entry_of(kern), "kernel module")?;
        if let When::Steps { dt, keep, .. } = kern.when {
            let dt = f32::from_bits(dt.0);
            if !(dt.is_finite() && dt > 0.0) || keep == loft_ir::bundle::Keep::Every(0) {
                return Err(bad(format!(
                    "kernel {k} steps by {dt} keeping {keep:?}; dt must be positive and Every(k) k > 0"
                )));
            }
            if kern.writes.len() != 1 || !matches!(b.resources[kern.writes[0] as usize].shape, Shape::Rows { .. }) {
                return Err(bad(format!("stepped kernel {k} must write exactly one rows resource")));
            }
        }
    }
    for d in b.draws.iter() {
        at(&b.names, d.name, "draw name")?;
        entry(d.shade, "draw module")?;
        match &d.cover {
            Cover::Raster { prim: RasterPrim::Mesh { res }, .. } => rows(b, *res, "mesh")?,
            Cover::Ray { proxy: Proxy::Instances { res, .. }, .. } => rows(b, *res, "ray instances")?,
            Cover::Raster { prim: RasterPrim::Grid { .. } | RasterPrim::Quads, .. }
            | Cover::Ray { proxy: Proxy::Box { .. }, .. }
            | Cover::Screen => {}
        }
        match &d.cover {
            Cover::Ray { find: Find::Medium(m), .. } => at(&b.media, *m, "medium").map(|_| ())?,
            Cover::Ray { find: Find::Hit, .. } | Cover::Raster { .. } | Cover::Screen => {}
        }
    }
    for p in b.passes.iter() {
        if p.draws[0] > p.draws[1] || p.draws[1] as usize > b.draws.len() {
            return Err(bad(format!("pass draws {:?} are not a range of the {} draws", p.draws, b.draws.len())));
        }
        let flat = |r: Id, what: &str| match b.resources[r as usize].shape {
            Shape::Texture { extent, .. } if extent[2] > 1 => {
                Err(bad(format!("{what} {r} is 3D; passes draw into 2D")))
            }
            Shape::Texture { .. } | Shape::ViewSized { .. } | Shape::Rows { .. } | Shape::Frame => Ok(()),
        };
        match p.target {
            Target::View => {}
            Target::Texture(r) => texture(b, r, false, "pass target").and_then(|_| flat(r, "pass target"))?,
            Target::Shadow { map, .. } => texture(b, map, true, "shadow map").and_then(|_| flat(map, "shadow map"))?,
        }
        for e in p.compose.outline.iter().chain(&p.compose.ground) {
            entry(*e, "compose module")?;
        }
    }
    for m in b.media.iter() {
        entry(m.transmittance, "medium module")?;
        if let loft_ir::bundle::Integrate::Gaussians { res } = m.integrate {
            rows(b, res, "gaussians")?;
        }
    }
    for i in b.inputs.iter() {
        let name = at(&b.names, i.name, "input name")?;
        // Main's wire (lr-wire) has no record type: every Ty is a scalar or vector, and `lits` is its float count.
        if i.def.len() != usize::from(i.ty.lits()) {
            return Err(bad(format!("input {name} is {:?} with {} default floats", i.ty, i.def.len())));
        }
    }
    let frame = loft_ir::frame::HEADER as u64 + 16 * (b.params.len().div_ceil(4).max(1) + b.inputs.len().max(1)) as u64;
    if frame > limits.max_uniform_buffer_binding_size {
        return Err(bad(format!(
            "the frame uniform needs {frame} bytes; this device binds {}: fewer params or inputs",
            limits.max_uniform_buffer_binding_size
        )));
    }
    for p in b.params.iter() {
        at(&b.names, p.name, "param name")?;
    }
    for c in b.cameras.iter() {
        at(&b.names, c.name, "camera name")?;
        entry(c.lens, "camera lens module")?;
    }
    packs(b, limits)?;
    for (k, m) in b.modules.iter().enumerate() {
        let text = std::str::from_utf8(at(&b.sections, m.wgsl, "module text")?)
            .map_err(|e| bad(format!("module {k}'s text is not UTF-8: {e}")))?;
        let pre = prelude(m, precision);
        let src = format!("{pre}{text}");
        let refuse = |line: Option<u32>, msg: String| {
            let line = line.unwrap_or(0).saturating_sub(pre.lines().count() as u32);
            match m.origin {
                // The bundle's site map has no line table for author modules yet; the line is the module's own.
                Origin::Author => Error::Scene { site: None, fault: SceneFault::Wgsl { line, msg: msg.into() } },
                Origin::Library => {
                    bad(format!("library module {k} line {line}: {msg}. A Loft bug: rebuild with the current compiler"))
                }
            }
        };
        let module = naga::front::wgsl::parse_str(&src)
            .map_err(|e| refuse(e.location(&src).map(|l| l.line_number), e.message().into()))?;
        let caps = naga::valid::Capabilities::default() | naga::valid::Capabilities::SHADER_FLOAT16;
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps)
            .validate(&module)
            .map_err(|e| refuse(e.location(&src).map(|l| l.line_number), e.as_inner().to_string()))?;
    }
    Ok(())
}
