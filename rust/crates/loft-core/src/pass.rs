//! Link a pass-local set of readings and keep the constructor arguments in a pack.
use super::{Build, Naga, unsupported};
use loft_ir::Error;
use loft_ir::bundle::{Access, Bundle, Format, Init, Module, Origin, Pack, Resource, Shape};
use loft_ir::pass::{Declarations, Open};

fn dispatch(open: Open) -> Result<String, Error> {
    let mut cases = open.cases;
    cases.sort_by_key(|c| c.0);
    if cases.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(unsupported(format!("{} has duplicate cases; assign each kind a distinct tag", open.signature)));
    }
    let mut text = format!("\n{} {{\n  switch {} {{\n", open.signature, open.selector);
    for (tag, expr) in cases {
        text.push_str(&format!("    case {tag}u: {{ return {expr}; }}\n"));
    }
    text.push_str(&format!("    default: {{ return {}; }}\n  }}\n}}\n", open.otherwise));
    Ok(text)
}

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
        let mut text = src.text;
        for open in src.opens {
            text.push_str(&dispatch(open)?);
        }
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
    for (i, t) in d.tables.into_iter().enumerate() {
        let stride = usize::from(t.lanes) * 4;
        if stride == 0 || t.words.is_empty() || !t.words.len().is_multiple_of(stride) {
            return Err(unsupported(format!("table {i}: supply nonempty whole rows of {} vec4s", t.lanes)));
        }
        let rows = u32::try_from(t.words.len() / stride)
            .map_err(|_| unsupported("table exceeds u32 rows; split it".into()))?;
        packs.push(Pack { access: Access::Read, vec4s: t.words.len() as u64 / 4, holds: Box::new([(i as u32, 0)]) });
        resources
            .push(Resource { shape: Shape::Rows { rows, lanes: t.lanes }, init: Init::Bytes(sections.len() as u32) });
        sections.push(t.words.into_iter().flat_map(u32::to_le_bytes).collect());
    }
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
