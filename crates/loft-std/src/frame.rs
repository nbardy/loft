//! Resolve immutable frame templates and apply the declared clock at each pass boundary.
use crate::error::*;
use loft_ir::*;
use std::collections::BTreeMap;

pub enum Templates {
    Clock,
    Tables {
        offset: usize,
        main: TableId,
        variants: BTreeMap<PassId, TableId>,
        bytes: BTreeMap<TableId, Box<[u8]>>,
    },
}

impl Templates {
    pub fn new(b: &Bundle, size: usize) -> Result<Self> {
        let Some(f) = &b.resources.frame else {
            return Ok(Self::Clock);
        };
        let offset = f.time_offset as usize;
        if offset.checked_add(4).is_none_or(|end| end > size) {
            return Err(RunnerError::AbiMismatch);
        }
        let get = |id| {
            let t = b
                .tables
                .iter()
                .find(|t| t.id == id)
                .ok_or(RunnerError::UnknownResource)?;
            if t.initial.len() != size {
                return Err(RunnerError::AbiMismatch);
            }
            Ok((id, t.initial.clone()))
        };
        let bytes = std::iter::once(f.main)
            .chain(f.variants.iter().map(|(_, t)| *t))
            .map(get)
            .collect::<Result<_>>()?;
        Ok(Self::Tables {
            offset,
            main: f.main,
            variants: f.variants.iter().copied().collect(),
            bytes,
        })
    }

    pub fn change(&self, id: PassId, previous: &mut Option<TableId>, time: f32) -> Option<Vec<u8>> {
        match self {
            Self::Clock => None,
            Self::Tables {
                offset,
                main,
                variants,
                bytes,
            } => {
                let table = variants.get(&id).unwrap_or(main);
                if previous.replace(*table) == Some(*table) {
                    return None;
                }
                let mut data = bytes[table].to_vec();
                data[*offset..*offset + 4].copy_from_slice(&time.to_le_bytes());
                Some(data)
            }
        }
    }
}
