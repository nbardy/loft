use loft_passes::program::Program;
use std::{error::Error, path::Path};

pub(crate) fn order_cull(program: &mut Program, root: &Path) -> Result<(), Box<dyn Error>> {
    // Equal D32 depths retain the first draw. Give compact cull output the same
    // stable row order as the existing control, preserving the arm's frustum test.
    // The stdlib cull fills its `draw` hole with one of these two fragments. This file is
    // copied out of the checkout to build the capture instrument, so read them from `root`.
    let compact_tail = &std::fs::read_to_string(root.join("stdlib/core/cull_multi.wgsl"))?;
    let indexed_tail = &std::fs::read_to_string(root.join("stdlib/core/cull_single.wgsl"))?;
    let index = program.module_named("cull").ok_or("missing cull module")?;
    let shader = &mut program.modules[index];
    match (
        shader.wgsl.matches(compact_tail.as_str()).count(),
        shader.wgsl.matches(indexed_tail.as_str()).count(),
    ) {
        (1, 0) => {
            shader.wgsl = shader.wgsl.replace(compact_tail, indexed_tail);
            shader.naga = naga::front::wgsl::parse_str(&shader.wgsl)?;
        }
        (0, 1) => {}
        _ => return Err("unrecognized cull ordering for deterministic pictures".into()),
    }
    Ok(())
}
