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
            motion:vec![],
            modules: vec![ShaderModule {
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
            sun: [0.; 3],sun_color:[1.;3],sky:None,
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
