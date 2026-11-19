//! The fixed group-0 parameter buffer and its deferred upload.
use super::*;

pub(super) use loft_std::params::Params;
impl From<loft_std::params::Error> for RunnerError {
    fn from(e: loft_std::params::Error) -> Self {
        match e {
            loft_std::params::Error::AbiMismatch => Self::AbiMismatch,
            loft_std::params::Error::Capacity => Self::Capacity,
        }
    }
}

impl Runner {
    pub fn params(&self, inst: InstanceId) -> &[ParamMeta] {
        self.instances
            .get(inst.0 as usize)
            .map_or(&[], |i| i.decl.params.as_slice())
    }

    pub fn set_param(&mut self, inst: InstanceId, id: ParamId, values: &[f32]) -> Result<()> {
        let inst = self.instance(inst)?;
        let meta = inst
            .decl
            .params
            .iter()
            .find(|p| p.id == id)
            .ok_or(RunnerError::UnknownResource)?;
        Ok(inst.compute.params.set(meta, values)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l23a_clamps_and_uploads_once() {
        let scalar = ParamMeta {
            id: ParamId(0),
            name: "roughness".into(),
            ty: IrTy::F32,
            offset_bytes: 0,
            default: [0.5; 4],
            range: Some(([0.25; 4], [0.75; 4])),
        };
        let vector = ParamMeta {
            id: ParamId(1),
            name: "direction".into(),
            ty: IrTy::Vec3,
            offset_bytes: 16,
            default: [0.; 4],
            range: Some(([-1.; 4], [1.; 4])),
        };
        let mut p = Params::new(&[scalar.clone(), vector.clone()], 64).unwrap();
        assert!(p.take_upload().is_none());
        p.set(&scalar, &[9.]).unwrap();
        p.set(&vector, &[3., 0.5, -2.]).unwrap();
        let before = p.bytes().to_vec();
        assert_eq!(
            p.set(&vector, &[0., f32::NAN, 1.]),
            Err(loft_std::params::Error::AbiMismatch)
        );
        assert_eq!(p.bytes(), before);
        let words: Vec<_> = p
            .take_upload()
            .unwrap()
            .chunks_exact(4)
            .map(|x| f32::from_le_bytes(x.try_into().unwrap()))
            .collect();
        assert_eq!(words, [0.75, 0., 0., 0., 1., 0.5, -1., 0.]);
        p.set(&scalar, &[10.]).unwrap();
        assert!(p.take_upload().is_none());
    }
}
