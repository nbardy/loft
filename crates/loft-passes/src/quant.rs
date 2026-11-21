pub(crate) use loft_emit::quant::wgsl_prelude;

/// Compile using the supplied host capabilities. The ordinary compiler
/// entry point targets the portable baseline.
pub fn compile(
    sources: &crate::source::Graph,
    overrides: &[(String, f64)],
    caps: loft_ir::HostCaps,
) -> loft_ir::Res<(crate::program::Program, crate::Report, crate::Compiled)> {
    crate::compile_impl(
        sources,
        overrides,
        &[],
        std::path::Path::new("target/loft_cache"),
        crate::emit::Aov::Beauty,
        caps.features,
    )
}
