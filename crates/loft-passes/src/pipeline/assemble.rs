use super::{lower_passes::LoweredPasses, schedule::Schedule, stage0::Stage0Scene};
use crate::{Compiled, Prog, Report, bsdf, eval, gltf, reflect_params};
use crate::program::Program;
use loft_ir::Res;
use loft_parse::ast::{Module, Ty};

pub(super) struct Assembled {
    program: Program,
    report: Report,
    drawn: Vec<(usize, String)>,
    compute: crate::AuthoredCompute,
    tables: eval::Tables,
    luts: Vec<eval::Lut>,
}

pub(super) fn assemble(
    p: &Prog<'_>, types: &[Ty], schedule: Schedule<'_>, scene: Stage0Scene,
    lowered: LoweredPasses, ms: Vec<(&'static str, f64)>,
) -> Res<Assembled> {
    let Schedule { drawn, compute, path, camera, sun, shadow_size, shadow_clock, .. } = schedule;
    let LoweredPasses { bakes, dispatches, shader_modules, passes, store_energy } = lowered;
    let Stage0Scene { tables, luts, params, mut lut_words, kinds, instances, reports, .. } = scene;
    let param_ranges = reflect_params(p, types, &params)?.into_iter().map(|m|
        (m.name, m.range.map(|(lo, hi)| [lo[0], hi[0]]))).collect();
    if store_energy { lut_words.extend(bsdf::energy_words()); }
    let program = Program {
        param_ranges,
        bakes,
        path,
        dispatches,
        modules: shader_modules,
        kinds,
        instances,
        camera,
        sun,
        params,
        shadow_size,
        luts: lut_words,
        passes,
        shadow_clock,
    };
    let drawn = drawn.iter().map(|d| (p.kind_index(d), d.to_string())).collect();
    Ok(Assembled { program, report: Report { kinds: reports, ms }, drawn, compute, tables, luts })
}

impl Assembled {
    pub(super) fn finish(
        self, modules: Vec<Module>, types: Vec<Ty>, meshes: Vec<gltf::Mesh>,
    ) -> (Program, Report, Compiled) {
        let Self { program, report, drawn, compute, tables, luts } = self;
        (program, report, Compiled { modules, types, tables, luts, drawn, meshes, compute })
    }
}
