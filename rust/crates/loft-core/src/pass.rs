//! Link a pass-local set of readings and keep the constructor arguments in a pack.
use super::{Build, Naga, unsupported};
use loft_ir::Error;
use loft_ir::bundle::{Access, Bundle, Format, Init, Module, Origin, Pack, Resource, Shape};
use loft_ir::bundle::{Body, Kernel, When};
use loft_ir::pass::{Declarations, Table};

pub fn link(b: &mut Bundle, mut d: Declarations, build: Build) -> Result<usize, Error> {
    if !b.packs.is_empty() || !b.passes.is_empty() {
        return Err(unsupported(
            "an ordered program with legacy draws or packs; declare its tables in the program".into(),
        ));
    }
    let mut sections = std::mem::take(&mut b.sections).into_vec();
    let mut modules = std::mem::take(&mut b.modules).into_vec();
    let mut total = 0;
    let offset = modules.len() as u32;
    for src in d.modules {
        let text = crate::link_source(src)?;
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
    let mut packs = vec![];
    let mut resources = vec![];
    let mut names = std::mem::take(&mut b.names).into_vec();
    let mut kernels = vec![];
    for (i, t) in d.tables.into_iter().enumerate() {
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
                kernels.push(Kernel { when, reads: Box::new([]), writes: Box::new([i as u32]), body });
                (lanes, rows, Access::Write, init)
            }
        };
        packs.push(Pack { access, vec4s: u64::from(rows) * u64::from(lanes), holds: Box::new([(i as u32, 0)]) });
        resources.push(Resource { shape: Shape::Rows { rows, lanes }, init });
    }
    b.names = names.into();
    b.kernels = kernels.into();
    for p in &mut d.schedule.passes {
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
