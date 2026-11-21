//! Material rows use the same stage-0 values and word packing as drawable rows.
use crate::{Prog, check::Ck, eval::Tables};
use loft_ir::{Code, Diag, KindId, Res};
use loft_parse::ast::{FnDecl, KindDecl, QueryKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub(crate) const BUFFER_PREFIX: &str = "loft_material_rows_";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialTable {
    pub name: String,
    pub kind_index: u32,
    pub stride_words: u32,
    pub columns: Vec<(String, u32)>,
    #[serde(with = "crate::sexp::words")]
    pub rows: Vec<f32>,
}

impl MaterialTable {
    /// Disjoint from the draw-kind IDs stored in Vis64.
    pub fn arena(&self) -> KindId {
        KindId(self.kind_index | 0x8000_0000)
    }

    pub fn row_count(&self) -> u32 {
        (self.rows.len() / self.stride_words as usize) as u32
    }

    pub(crate) fn binding(&self) -> (u32, u32) {
        binding(self.kind_index)
    }
}

pub(crate) fn binding(kind_index: u32) -> (u32, u32) {
    (1, 4 + kind_index)
}

pub(crate) fn stride(words: u32) -> u32 {
    words.max(16).next_multiple_of(4)
}

pub(crate) fn row_words(p: &Prog<'_>, kind: &KindDecl) -> Res<u32> {
    let mut types = Vec::new();
    let ck = Ck::new(p, &mut types);
    kind.row
        .iter()
        .map(|field| {
            ck.resolve(&field.ty, true)?.words().ok_or_else(|| {
                Diag::new(
                    Code::E003,
                    field.name.span,
                    "material column cannot be packed",
                    "use a storable row type",
                )
            })
        })
        .sum()
}

pub(crate) fn used_by<'a>(p: &Prog<'_>, query: &'a FnDecl) -> Vec<&'a str> {
    crate::emit::References::function(query)
        .names
        .into_iter()
        .filter_map(|(name, _)| {
            p.kinds
                .get(name)
                .filter(|kind| kind.queries.is_empty())
                .map(|_| name)
        })
        .collect()
}

pub(crate) fn pack_tables(
    p: &Prog<'_>,
    tables: &Tables,
    drawn: &[&str],
) -> Res<Vec<MaterialTable>> {
    let used = drawn
        .iter()
        .filter_map(|name| p.kinds[*name].query(QueryKind::Material))
        .flat_map(|query| used_by(p, query))
        .collect::<BTreeSet<_>>();
    let mut types = Vec::new();
    let ck = Ck::new(p, &mut types);
    used.into_iter()
        .map(|name| {
            let kind = p.kinds[name];
            let index = p.kind_index(name);
            let source = &tables[index];
            let mut words = 0;
            let columns = kind
                .row
                .iter()
                .map(|column| {
                    let offset = words;
                    words += ck
                        .resolve(&column.ty, true)?
                        .words()
                        .expect("checked storable field");
                    Ok((column.name.name.clone(), offset))
                })
                .collect::<Res<_>>()?;
            let stride_words = stride(words);
            if source.is_empty() || source.len() > (u32::MAX / 4 / stride_words) as usize {
                return Err(Diag::new(
                    Code::E602,
                    kind.name.span,
                    "material table extent is out of range",
                    "emit at least one material and keep its table under 4 GiB",
                ));
            }
            let mut rows = Vec::with_capacity(source.len() * stride_words as usize);
            for row in source {
                let end = rows.len() + stride_words as usize;
                crate::pack(row, &mut rows);
                rows.resize(end, 0.0);
            }
            Ok(MaterialTable {
                name: name.to_owned(),
                kind_index: index as u32,
                stride_words,
                columns,
                rows,
            })
        })
        .collect()
}

pub(crate) fn validate(tables: &[MaterialTable]) -> Result<(), String> {
    let mut indices = BTreeSet::new();
    for t in tables {
        if t.kind_index >= 0x8000_0000
            || !indices.insert(t.kind_index)
            || !(16..=32).contains(&t.stride_words)
            || !t.stride_words.is_multiple_of(4)
            || t.rows.is_empty()
            || t.rows.len() > (u32::MAX / 4) as usize
            || !t.rows.len().is_multiple_of(t.stride_words as usize)
            || t.columns
                .iter()
                .any(|(_, offset)| *offset >= t.stride_words)
        {
            return Err(format!("invalid packed material table {}", t.name));
        }
    }
    Ok(())
}
