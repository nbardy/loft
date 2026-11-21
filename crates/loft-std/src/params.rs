//! Packed parameter shadow and deferred writes.
use loft_ir::{IrTy, ParamMeta};

pub use crate::error::RunnerError as Error;

pub struct Params {
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
    pub fn new(meta: &[ParamMeta], max_bytes: u64) -> std::result::Result<Self, Error> {
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
                return Err(Error::AbiMismatch);
            }
            let end = offset
                .checked_add(n * 4)
                .filter(|&x| x.next_multiple_of(16) as u64 <= max_bytes)
                .ok_or(Error::Capacity)?;
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

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn set(&mut self, p: &ParamMeta, values: &[f32]) -> std::result::Result<(), Error> {
        let n = width(p.ty);
        if values.len() != n || values.iter().any(|v| !v.is_finite()) {
            return Err(Error::AbiMismatch);
        }
        let mut words = [[0; 4]; 4];
        for (i, &value) in values.iter().enumerate() {
            let value = if let Some((lo, hi)) = p.range {
                if !lo[i].is_finite() || !hi[i].is_finite() || lo[i] > hi[i] {
                    return Err(Error::AbiMismatch);
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
                IrTy::U32 | IrTy::Bool => return Err(Error::AbiMismatch),
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

    pub fn copy_from(&mut self, previous: &Self) {
        self.bytes.copy_from_slice(&previous.bytes);
        self.dirty = true;
    }

    pub fn take_upload(&mut self) -> Option<&[u8]> {
        std::mem::take(&mut self.dirty).then_some(&self.bytes)
    }
}
