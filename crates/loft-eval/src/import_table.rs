//! Stage-0 sampled row assets. See docs/spec/07_PLATFORM_POWER_EXTENSIONS.md.
use crate::{Prog, eval::RowVal};
use loft_check::{check::Ck, source::Graph};
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::{KindDecl, Ty};
use std::{fs, path::Path};

const MAGIC: &[u8; 8] = b"LOFTTAB1";
pub(crate) type Rows = Vec<Vec<Option<RowVal>>>;

pub(crate) fn error(span: Span, message: impl Into<String>) -> Diag {
    Diag::new(Code::E602, span, message, "supply a valid binary table matching the kind's declared columns")
}

pub(crate) fn load(p: &Prog, kind: &KindDecl, source: &Graph, path: &str, span: Span) -> Res<Rows> {
    let path: String = serde_json::from_str(path).map_err(|e| error(span, format!("invalid table path: {e}")))?;
    let relative = Path::new(&path);
    if relative.extension().is_none_or(|ext| ext != "bin") || relative.is_absolute() || path.contains(':') {
        return Err(error(span, "expected a relative .bin table path"));
    }
    let file = source.source_path(span.file).with_file_name(relative);
    let bytes = fs::read(&file).map_err(|e| error(span, format!("{}: {e}", file.display())))?;
    let mut unused = vec![];
    let checker = Ck::new(p, &mut unused);
    let schema = kind.row.iter().map(|field| {
        let layout = match checker.resolve(&field.ty, true)? {
            Ty::F32 => Column::Float(1),
            Ty::VecN(n @ 2..=4) => Column::Float(n),
            Ty::U32 => Column::Unsigned,
            Ty::I32 => Column::Signed,
            Ty::Bool => Column::Bool,
            ty => return Err(error(field.ty.span(), format!("binary tables do not support {ty} columns"))),
        };
        Ok((field.name.name.as_str(), layout, field.default.is_some()))
    }).collect::<Res<Vec<_>>>()?;
    read(&bytes, &schema).map_err(|e| error(span, format!("{}: {e}", file.display())))
}

#[derive(Clone, Copy)]
enum Column {
    Float(u8),
    Unsigned,
    Signed,
    Bool,
}

impl Column {
    fn tag(self) -> u8 {
        match self {
            Self::Float(lanes) => lanes,
            Self::Unsigned => 5,
            Self::Signed => 6,
            Self::Bool => 7,
        }
    }

    fn read(self, bytes: &mut &[u8]) -> Result<RowVal, &'static str> {
        let float = |bytes: &mut &[u8]| {
            let value = f32::from_le_bytes(take(bytes, 4)?.try_into().unwrap());
            value.is_finite().then_some(value).ok_or("nonfinite float")
        };
        Ok(match self {
            Self::Float(1) => RowVal::F(float(bytes)?),
            Self::Float(n) => RowVal::V((0..n).map(|_| float(bytes)).collect::<Result<_, _>>()?),
            Self::Unsigned => RowVal::U(u32::from_le_bytes(take(bytes, 4)?.try_into().unwrap())),
            Self::Signed => RowVal::I(i32::from_le_bytes(take(bytes, 4)?.try_into().unwrap())),
            Self::Bool => RowVal::B(match take(bytes, 1)?[0] {
                0 => false,
                1 => true,
                _ => return Err("invalid bool"),
            }),
        })
    }
}

fn take<'a>(bytes: &mut &'a [u8], n: usize) -> Result<&'a [u8], &'static str> {
    let (head, rest) = bytes.split_at_checked(n).ok_or("truncated table")?;
    *bytes = rest;
    Ok(head)
}

fn read(mut bytes: &[u8], schema: &[(&str, Column, bool)]) -> Result<Rows, String> {
    if take(&mut bytes, MAGIC.len())? != MAGIC {
        return Err("invalid table magic or version".into());
    }
    let rows = u32::from_le_bytes(take(&mut bytes, 4)?.try_into().unwrap()) as usize;
    let cols = u16::from_le_bytes(take(&mut bytes, 2)?.try_into().unwrap()) as usize;
    if cols == 0 || cols != schema.len() {
        return Err(format!("header declares {cols} columns, kind declares {}", schema.len()));
    }
    for (name, layout, _) in schema {
        let len = take(&mut bytes, 1)?[0] as usize;
        let actual = take(&mut bytes, len)?;
        let tag = take(&mut bytes, 1)?[0];
        if name.as_bytes() != actual || tag != layout.tag() {
            return Err(format!("column layout differs at {name} (expected tag {})", layout.tag()));
        }
    }
    let mask_len = cols.div_ceil(8);
    if rows > bytes.len() / mask_len {
        return Err("row count exceeds payload capacity".into());
    }
    let mut result = Vec::with_capacity(rows);
    for index in 0..rows {
        let mask = take(&mut bytes, mask_len)?;
        if cols % 8 != 0 && mask[mask_len - 1] >> (cols % 8) != 0 {
            return Err(format!("row {index} has bits outside the declared columns"));
        }
        let row = schema.iter().enumerate().map(|(i, (name, layout, has_default))| {
            if mask[i / 8] & (1 << (i % 8)) != 0 {
                layout.read(&mut bytes).map(Some).map_err(|e| format!("row {index}, column {name}: {e}"))
            } else if *has_default {
                Ok(None)
            } else {
                Err(format!("row {index} omits required column {name}"))
            }
        }).collect::<Result<Vec<_>, _>>()?;
        result.push(row);
    }
    if !bytes.is_empty() {
        return Err("trailing bytes after the declared rows".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_corrupt_sample_tables_and_preserves_sparse_defaults() {
        let schema = [("x", Column::Float(1), false), ("y", Column::Float(3), true)];
        let mut bytes = b"LOFTTAB1\x01\0\0\0\x02\0\x01x\x01\x01y\x03\x01".to_vec();
        bytes.extend_from_slice(&(-0.0f32).to_le_bytes());
        let rows = read(&bytes, &schema).unwrap();
        assert!(matches!(rows[0].as_slice(), [Some(RowVal::F(x)), None] if x.is_sign_negative()));
        assert!(read(&bytes[..bytes.len() - 1], &schema).unwrap_err().contains("truncated"));
        assert!(read(&bytes, &[("q", Column::Float(1), false), schema[1]]).unwrap_err().contains("layout"));
        bytes[0] = 0;
        assert!(read(&bytes, &schema).unwrap_err().contains("magic"));
    }
}
