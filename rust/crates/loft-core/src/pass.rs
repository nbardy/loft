//! Link a pass-local set of readings and keep the constructor arguments in a pack.
use super::{Build, Naga, unsupported};
use loft_ir::Error;
use loft_ir::bundle::{Access, Bundle, Format, Init, Module, Origin, Pack, Resource, Shape};
use loft_ir::bundle::{Body, Kernel, When};
use loft_ir::pass::{Declarations, Table};

pub fn link(
    b: &mut Bundle,
    mut d: Declarations,
    build: Build,
    g: &loft_ir::graph::Graph,
    slots: &crate::emit::Slots,
) -> Result<usize, Error> {
    if !b.passes.is_empty() {
        return Err(unsupported("an ordered program with legacy draws; choose one pass list".into()));
    }
    let mut sections = std::mem::take(&mut b.sections).into_vec();
    let mut modules = std::mem::take(&mut b.modules).into_vec();
    let mut total = 0;
    let offset = modules.len() as u32;
    let resource_base = b.resources.len() as u32;
    let pack_base = b.packs.len() as u32;
    for (i, src) in d.modules.into_iter().enumerate() {
        let text = crate::link_source(src)?;
        let text = if pack_base == 0 {
            text
        } else {
            let stage = if d.tables.iter().any(|t| matches!(t,Table::Made{module,..} if *module==i as u32)) {
                crate::emit::Stage::Compute
            } else {
                crate::emit::Stage::Render
            };
            attach(&text, g, slots, stage, pack_base)?
        };
        if let Naga::Emit = build.naga {
            let module = naga::front::wgsl::parse_str(&text).map_err(|e| unsupported(e.emit_to_string(&text)))?;
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
                .validate(&module)
                .map_err(|e| unsupported(e.emit_to_string(&text)))?;
        }
        total += text.len();
        modules.push(Module { wgsl: sections.len() as u32, origin: Origin::Author, precision_alias: false });
        sections.push(text.into_bytes().into());
    }
    let mut packs = std::mem::take(&mut b.packs).into_vec();
    let mut resources = std::mem::take(&mut b.resources).into_vec();
    let mut names = std::mem::take(&mut b.names).into_vec();
    let mut kernels = std::mem::take(&mut b.kernels).into_vec();
    for (i, t) in d.tables.into_iter().enumerate() {
        let resource = resource_base + i as u32;
        let (lanes, rows, access, init) = match t {
            Table::Data { lanes, words } => {
                let stride = usize::from(lanes) * 4;
                if stride == 0 || words.is_empty() || !words.len().is_multiple_of(stride) {
                    return Err(unsupported(format!("table {i}: supply nonempty whole rows of {lanes} vec4s")));
                }
                let rows = u32::try_from(words.len() / stride)
                    .map_err(|_| unsupported("table exceeds u32 rows; split it".into()))?;
                let init = Init::Bytes(sections.len() as u32);
                sections.push(words.into_iter().flat_map(u32::to_le_bytes).collect());
                (lanes, rows, Access::Read, init)
            }
            Table::Made { lanes, rows, module, entry, extent, when } => {
                let module = module
                    .checked_add(offset)
                    .filter(|&m| (m as usize) < modules.len())
                    .ok_or_else(|| unsupported(format!("table {i}: supply a declared fill module")))?;
                if lanes == 0 || rows == 0 || extent.contains(&0) {
                    return Err(unsupported(format!("table {i}: use nonzero row, lane and dispatch counts")));
                }
                let (init, when) = match when {
                    loft_ir::pass::Clock::Load => (Init::By(kernels.len() as u32), When::Load),
                    loft_ir::pass::Clock::Frame => (Init::Zero, When::Frame),
                };
                let body = Body::Raw { module, entry: names.len() as u32, dispatch: extent };
                names.push(entry.into());
                kernels.push(Kernel { when, reads: Box::new([]), writes: Box::new([resource]), body });
                (lanes, rows, Access::Write, init)
            }
        };
        packs.push(Pack { access, vec4s: u64::from(rows) * u64::from(lanes), holds: Box::new([(resource, 0)]) });
        resources.push(Resource { shape: Shape::Rows { rows, lanes }, init });
    }
    b.names = names.into();
    b.kernels = kernels.into();
    for p in &mut d.schedule.passes {
        let rebase = |r: &mut u32| -> Result<(), Error> {
            *r = r
                .checked_add(resource_base)
                .filter(|&id| (id as usize) < resources.len())
                .ok_or_else(|| unsupported(format!("{}: use a declared local row table", p.name)))?;
            Ok(())
        };
        if let loft_ir::pass::Run::Raster { draws } = &mut p.run {
            for draw in draws {
                if let Some(id) = &mut draw.indices {
                    rebase(id)?;
                }
                match &mut draw.instances {
                    loft_ir::pass::Count::Rows(id) | loft_ir::pass::Count::Indirect(id) => rebase(id)?,
                    loft_ir::pass::Count::Fixed(_) => {}
                }
            }
        }
        p.module = p
            .module
            .checked_add(offset)
            .filter(|&m| (m as usize) < modules.len())
            .ok_or_else(|| unsupported(format!("{}: supply a declared module", p.name)))?;
    }
    b.format = Format::V2(d.schedule);
    b.sections = sections.into();
    b.modules = modules.into();
    b.resources = resources.into();
    b.packs = packs.into();
    Ok(total)
}

/// Use Naga's binding identities to relocate the two pack spaces once. Shared packs can contain several
/// rows resources at different offsets. Their usual t_name accessor retains the graph compiler's layout.
fn attach(
    text: &str,
    g: &loft_ir::graph::Graph,
    slots: &crate::emit::Slots,
    stage: crate::emit::Stage,
    base: u32,
) -> Result<String, Error> {
    let mut source = crate::emit::packs(slots, stage).replace("@group(1)", "@group(4)");
    for (i, &row) in slots.kernels.iter().enumerate() {
        source.push_str(&crate::emit::accessor(g, i as u32, Some(row)));
        source.push('\n');
    }
    source.push_str(text);
    let mut module = naga::front::wgsl::parse_str(&source).map_err(|e| unsupported(e.emit_to_string(&source)))?;
    for (_, var) in module.global_variables.iter_mut() {
        if let Some(binding) = &mut var.binding {
            match binding.group {
                1 => {
                    binding.binding = binding
                        .binding
                        .checked_add(base)
                        .ok_or_else(|| unsupported("table binding overflow; split the pass".into()))?
                }
                4 if var.name.as_deref() == Some(format!("loft_pack{}", binding.binding).as_str()) => binding.group = 1,
                0..=3 => {}
                _ => {
                    return Err(unsupported(
                        "ordered modules use binding groups 0 through 3; read shared rows with their t_name function"
                            .into(),
                    ));
                }
            }
        }
    }
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
        .validate(&module)
        .map_err(|e| unsupported(e.emit_to_string(&source)))?;
    naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
        .map_err(|e| unsupported(format!("writing relocated module: {e}")))
}
