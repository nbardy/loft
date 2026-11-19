//! Canonical code-as-data form of the lowered program, including Naga's typed
//! expression arenas and resource bindings. Generated shader text is reconstructed.
use crate::program::Program;

pub fn write(program: &Program) -> Result<String, String> {
    serde_lexpr::to_string(&("loft-ir-v1", program)).map_err(|e| e.to_string())
}

pub fn read(text: &str) -> Result<Program, String> {
    let (version, mut p): (String, Program) = serde_lexpr::from_str(text)
        .map_err(|e| format!("invalid Loft IR: {e}; supply a v1 program from loft ir --sexp"))?;
    if version != "loft-ir-v1" {
        return Err(format!("unknown IR version {version}; use loft-ir-v1"));
    }
    for m in &mut p.modules {
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
        .validate(&m.naga)
        .map_err(|e| format!("invalid module {}: {e}", m.name))?;
        m.wgsl =
            naga::back::wgsl::write_string(&m.naga, &info, naga::back::wgsl::WriterFlags::empty())
                .map_err(|e| format!("cannot emit {}: {e}", m.name))?;
    }
    for k in &p.kinds {
        if k.module >= p.modules.len()
            || u64::from(k.stride) * u64::from(k.n_rows) != k.rows.len() as u64
        {
            return Err(format!(
                "invalid kind {}; restore its module index and row stride",
                k.name
            ));
        }
    }
    let valid_module = |i: usize| i < p.modules.len();
    if ![p.cull, p.classify, p.present]
        .into_iter()
        .all(valid_module)
    {
        return Err(
            "invalid shared module index; restore the cull, classify and present modules".into(),
        );
    }
    for pass in &p.passes {
        use crate::program::Pass;
        match pass {
            Pass::Vis(k) | Pass::Shadow(k) | Pass::Shade(k)
                if k.iter().any(|id| *id as usize >= p.kinds.len()) =>
            {
                return Err("invalid pass kind index; name a kind in the bundle".into());
            }
            Pass::Vis(_)
            | Pass::Shadow(_)
            | Pass::Shade(_)
            | Pass::Cull
            | Pass::Classify
            | Pass::Present => {}
        }
    }
    for instance in &p.instances {
        if !p
            .kinds
            .get(instance.kind as usize)
            .is_some_and(|k| instance.row < k.n_rows)
        {
            return Err("invalid instance; restore its kind and row indices".into());
        }
    }
    for step in &p.dispatches {
        let valid_table = step.input.view(&p.kinds).is_some_and(|(_, n, stride, words)| {
            n != 0 && stride != 0 && u64::from(n) * u64::from(stride) == words.len() as u64
        });
        let valid_entry = p.modules.get(step.module).is_some_and(|m| m.naga.entry_points.iter().any(|e|
            e.name == step.entry && e.stage == naga::ShaderStage::Compute && e.workgroup_size == [step.workgroup_x, 1, 1]));
        if !valid_table || !valid_entry || step.fields.is_empty() && step.checks.is_empty() {
            return Err(
                "invalid motion entry; supply an existing kind and its writes or checks".into(),
            );
        }
    }
    Ok(p)
}

/// A storage word can contain an integer bitcast to a float, including a NaN
/// payload. Decimal float serialization would lose that row's identity.
pub(crate) mod words {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer>(v: &[f32], s: S) -> Result<S::Ok, S::Error> {
        v.iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>()
            .serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f32>, D::Error> {
        Ok(Vec::<u32>::deserialize(d)?
            .into_iter()
            .map(f32::from_bits)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program::{Orbit, Pass, ShaderModule};
    #[test]
    fn typed_shader_round_trip() {
        let shader =
            "// secret author comment\n@compute @workgroup_size(1) fn main() { let x = 2u; }";
        let p = Program {
            dispatches: vec![],
            modules: vec![ShaderModule {
                origins: vec![],
                name: "example".into(),
                wgsl: shader.into(),
                naga: naga::front::wgsl::parse_str(shader).unwrap(),
                span: Some(("scene.loft".into(), 3, 4)),
            }],
            kinds: vec![],
            instances: vec![],
            camera: Orbit {
                at: [0.; 3],
                radius: 1.,
                height: 0.,
                period: 1.,
                fov: 1.,
            },
            sun: [0.; 3],
            params: vec![],
            shadow_size: 0,
            luts: vec![],
            passes: vec![Pass::Cull],
            cull: 0,
            classify: 0,
            present: 0,
        };
        let first = write(&p).unwrap();
        assert!(!first.contains("secret author comment"));
        assert_eq!(write(&read(&first).unwrap()).unwrap(), first);
    }
}
