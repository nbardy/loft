//! The fixed group-0 parameter buffer and its deferred upload.
use super::*;

pub(super) struct Params {
    bytes: Vec<u8>,
    dirty: bool,
}

fn width(ty: IrTy) -> usize {
    match ty {
        IrTy::F32 | IrTy::U32 | IrTy::Bool => 1,
        IrTy::Vec2 => 2,
        IrTy::Vec3 => 3,
        IrTy::Vec4 => 4,
    }
}

impl Params {
    pub(super) fn new(meta: &[ParamMeta], max_bytes: u64) -> Result<Self> {
        let mut params = Self {
            bytes: vec![0; 16],
            dirty: false,
        };
        let mut ids = std::collections::BTreeSet::new();
        let mut offsets = std::collections::BTreeSet::new();
        for p in meta {
            let n = width(p.ty);
            let offset = p.offset_bytes as usize;
            let alignment = (n * 4).next_power_of_two();
            if !offset.is_multiple_of(alignment)
                || !ids.insert(p.id)
                || (0..n).any(|i| !offsets.insert(offset + i * 4))
            {
                return Err(RunnerError::AbiMismatch);
            }
            let end = offset
                .checked_add(n * 4)
                .filter(|&x| x.next_multiple_of(16) as u64 <= max_bytes)
                .ok_or(RunnerError::Capacity)?;
            params.bytes.resize(params.bytes.len().max(end), 0);
            params.set(p, &p.default[..n])?;
        }
        // An array<vec4f> shader can only address complete 16-byte slots.
        params
            .bytes
            .resize(params.bytes.len().next_multiple_of(16), 0);
        params.dirty = false;
        Ok(params)
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn set(&mut self, p: &ParamMeta, values: &[f32]) -> Result<()> {
        let n = width(p.ty);
        if values.len() != n || values.iter().any(|v| !v.is_finite()) {
            return Err(RunnerError::AbiMismatch);
        }
        let mut words = [[0; 4]; 4];
        for (i, &value) in values.iter().enumerate() {
            let value = if let Some((lo, hi)) = p.range {
                if !lo[i].is_finite() || !hi[i].is_finite() || lo[i] > hi[i] {
                    return Err(RunnerError::AbiMismatch);
                }
                value.clamp(lo[i], hi[i])
            } else {
                value
            };
            words[i] = match p.ty {
                IrTy::U32 if value >= 0.0 && value < 4294967296.0 && value.fract() == 0.0 => {
                    (value as u32).to_le_bytes()
                }
                IrTy::Bool if matches!(value, 0.0 | 1.0) => u32::from(value != 0.0).to_le_bytes(),
                IrTy::F32 | IrTy::Vec2 | IrTy::Vec3 | IrTy::Vec4 => value.to_le_bytes(),
                IrTy::U32 | IrTy::Bool => return Err(RunnerError::AbiMismatch),
            };
        }
        let start = p.offset_bytes as usize;
        let bytes = words[..n].as_flattened();
        let slot = &mut self.bytes[start..start + bytes.len()];
        if slot != bytes {
            slot.copy_from_slice(bytes);
            self.dirty = true;
        }
        Ok(())
    }

    pub(super) fn copy_from(&mut self, previous: &Self) {
        self.bytes.copy_from_slice(&previous.bytes);
        self.dirty = true;
    }

    pub(super) fn take_upload(&mut self) -> Option<&[u8]> {
        std::mem::take(&mut self.dirty).then_some(&self.bytes)
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
        inst.compute.params.set(meta, values)
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
        let before = p.bytes.clone();
        assert_eq!(
            p.set(&vector, &[0., f32::NAN, 1.]),
            Err(RunnerError::AbiMismatch)
        );
        assert_eq!(p.bytes, before);
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
