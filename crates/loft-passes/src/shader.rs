use crate::{bsdf, provenance, quant};
use crate::program::ShaderModule;
use loft_ir::{Code, Diag, Res, Sources, Span};

pub(super) fn naga_module(name: &str, text: String, src: &Sources, features: loft_ir::GpuFeatures) -> Res<ShaderModule> {
    let mut prelude = quant::wgsl_prelude(&text, features);
    if text.contains("loft_ggx_energy(") {
        let table = if text.contains("var<storage, read> world_luts:") { Some("world_luts") }
            else if text.contains("var<storage, read> luts:") { Some("luts") }
            else { None };
        prelude.push_str(&bsdf::energy_wgsl(table));
    }
    prelude.push_str(&text);
    let map=provenance::Map::new(prelude);
    let diagnostic=|at:Option<Span>,message:String|Diag::new(Code::E701,at.unwrap_or(Span::NONE),format!("WGSL validation in {name}: {message}"),"correct the WGSL expression at this source location");
    let module=naga::front::wgsl::parse_str(&map.shader).map_err(|e|diagnostic(e.labels().find_map(|(s,_)|map.origin(s)),e.emit_to_string(&map.shader)))?;
    let mut caps = naga::valid::Capabilities::default();
    caps.set(naga::valid::Capabilities::SUBGROUP, features.contains(loft_ir::GpuFeatures::SUBGROUPS));
    caps.set(naga::valid::Capabilities::SHADER_FLOAT32_ATOMIC, features.contains(loft_ir::GpuFeatures::FLOAT32_ATOMICS));
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps).validate(&module).map_err(|e|diagnostic(e.spans().find_map(|(s,_)|map.origin(*s)),e.emit_to_string(&map.shader)))?;
    let origins=map.expressions(&module,src);
    Ok(ShaderModule{name:name.into(),wgsl:map.shader,naga:module,span:None,origins})
}
