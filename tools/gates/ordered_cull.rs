use loft_passes::program::Program;
use std::error::Error;

pub(crate) fn order_cull(program: &mut Program) -> Result<(), Box<dyn Error>> {
    // Equal D32 depths retain the first draw. Give compact cull output the same
    // stable row order as the existing control, preserving the arm's frustum test.
    let shader = &mut program.modules[program.cull];
    let compact = loft_passes::emit::cull_module(false);
    let indexed = loft_passes::emit::cull_module(true);
    let boundary = "  if outside != 0u { return; }";
    let (prefix, _) = compact
        .split_once(boundary)
        .ok_or("missing compact cull tail")?;
    let indexed_tail = indexed
        .strip_prefix(prefix)
        .ok_or("unexpected indexed cull shader")?;
    let compact_tail = &compact[prefix.len()..];
    match (
        shader.wgsl.matches(compact_tail).count(),
        shader.wgsl.matches(indexed_tail).count(),
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
