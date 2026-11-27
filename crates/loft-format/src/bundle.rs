//! Compiler-free LOFTB001 IO. Shader validation and device setup are separate from file decoding.
use loft_ir::*;
use std::{collections::BTreeMap, fmt};
pub(super) mod wire;
use wire::{Reader, Wire, get_bytes, length, put_bytes, sum};

const MAGIC: &[u8; 8] = b"LOFTB001";
const COMPILER_ABI: u32 = 1;
const HAS_WGSL: u32 = 2;
const HEADER: usize = 56;
const ENTRY: usize = 56;
const TAGS: [u32; 3] = [1, 2, 3];
const ASSETS: u32 = 4;

#[derive(Debug, PartialEq, Eq)]
pub enum BundleError {
    Version,
    Truncated,
    Encoding,
    Hash,
    Shader(ModuleId, String),
}

impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Version => f.write_str("unsupported .loftb format, flags or compiler ABI"),
            Self::Truncated => f.write_str("truncated .loftb data"),
            Self::Encoding => f.write_str("invalid .loftb section or declaration encoding"),
            Self::Hash => f.write_str(".loftb checksum mismatch"),
            Self::Shader(id, e) => write!(f, "module {}: {e}", id.0),
        }
    }
}
impl std::error::Error for BundleError {}

pub fn encode(bundle: &Bundle) -> Vec<u8> {
    let mut manifest = Vec::new();
    // The two lengths and their declarations precede the rest of the manifest.
    put_metadata(&bundle.modules, &mut manifest);
    put_metadata(&bundle.tables, &mut manifest);
    bundle.struct_layouts.put(&mut manifest);
    (bundle.rows.len() as u64).put(&mut manifest);
    for (&kind, &table) in &bundle.rows {
        (kind, table).put(&mut manifest);
    }
    bundle.state.put(&mut manifest);
    bundle.params.put(&mut manifest);
    bundle.passes.put(&mut manifest);

    let mut sections = vec![
        (1, manifest),
        (2, bulk(bundle.tables.iter().map(|t| t.initial.as_ref()))),
        (3, bulk(bundle.modules.iter().map(|m| m.bytes.as_ref()))),
    ];
    if !bundle.resources.is_empty() {
        let mut assets = Vec::new();
        bundle.resources.put(&mut assets);
        sections.push((ASSETS, assets));
    }
    let mut bytes = vec![0; HEADER + ENTRY * sections.len()];
    let flags = shader_flags(&bundle.modules);
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8..12].copy_from_slice(&1_u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&flags.to_le_bytes());
    bytes[48..52].copy_from_slice(&COMPILER_ABI.to_le_bytes());
    bytes[52..56].copy_from_slice(&(sections.len() as u32).to_le_bytes());
    let mut digest = blake3::Hasher::new();
    for (i, (tag, data)) in sections.into_iter().enumerate() {
        let start = bytes.len().next_multiple_of(16);
        bytes.resize(start, 0);
        let entry = &mut bytes[HEADER + i * ENTRY..][..ENTRY];
        entry[..4].copy_from_slice(&tag.to_le_bytes());
        entry[8..16].copy_from_slice(&(start as u64).to_le_bytes());
        entry[16..24].copy_from_slice(&(data.len() as u64).to_le_bytes());
        entry[24..].copy_from_slice(blake3::hash(&data).as_bytes());
        digest.update(&data);
        bytes.extend_from_slice(&data);
    }
    bytes[16..48].copy_from_slice(digest.finalize().as_bytes());
    bytes
}

fn shader_flags(modules: &[ModuleBlob]) -> u32 {
    if modules
        .iter()
        .any(|module| module.target == ShaderTarget::Wgsl)
    {
        HAS_WGSL
    } else {
        0
    }
}

fn bulk<'a>(data: impl ExactSizeIterator<Item = &'a [u8]>) -> Vec<u8> {
    let mut out = Vec::new();
    (data.len() as u64).put(&mut out);
    for bytes in data {
        put_bytes(bytes, &mut out);
    }
    out
}

pub fn decode(bytes: &[u8]) -> Result<Bundle, BundleError> {
    let mut header = Reader::new(bytes);
    if header.take(8)? != MAGIC || u32::get(&mut header)? != 1 {
        return Err(BundleError::Version);
    }
    let flags = u32::get(&mut header)?;
    let expected = <[u8; 32]>::get(&mut header)?;
    if flags & !HAS_WGSL != 0 || u32::get(&mut header)? != COMPILER_ABI {
        return Err(BundleError::Version);
    }
    let count = u32::get(&mut header)? as usize;
    if !matches!(count, 3 | 4) {
        return Err(BundleError::Version);
    }
    let mut digest = blake3::Hasher::new();
    let mut end = HEADER + ENTRY * count;
    let mut sections = [&[][..]; 4];
    for (slot, tag) in sections[..count]
        .iter_mut()
        .zip(TAGS.into_iter().chain([ASSETS]))
    {
        if u32::get(&mut header)? != tag || u32::get(&mut header)? != 0 {
            return Err(BundleError::Encoding);
        }
        let offset = usize::try_from(u64::get(&mut header)?).map_err(|_| BundleError::Truncated)?;
        let len = usize::try_from(u64::get(&mut header)?).map_err(|_| BundleError::Truncated)?;
        let hash = <[u8; 32]>::get(&mut header)?;
        if offset != end.next_multiple_of(16)
            || bytes
                .get(end..offset)
                .ok_or(BundleError::Truncated)?
                .iter()
                .any(|&b| b != 0)
        {
            return Err(BundleError::Encoding);
        }
        end = offset.checked_add(len).ok_or(BundleError::Truncated)?;
        *slot = bytes.get(offset..end).ok_or(BundleError::Truncated)?;
        if blake3::hash(slot).as_bytes() != &hash {
            return Err(BundleError::Hash);
        }
        digest.update(slot);
    }
    if end != bytes.len() {
        return Err(BundleError::Encoding);
    }
    if digest.finalize().as_bytes() != &expected {
        return Err(BundleError::Hash);
    }

    let [meta, packed_rows, packed_modules, assets] = sections;
    let resources = if count == 4 {
        let mut reader = Reader::new(assets);
        let resources = Resources::get(&mut reader)?;
        reader.finish()?;
        if resources.is_empty() {
            return Err(BundleError::Encoding);
        }
        resources
    } else {
        Resources::default()
    };
    let mut reader = Reader::new(meta);
    let modules = get_metadata::<ModuleBlob>(&mut reader)?;
    let tables = get_metadata::<TableDecl>(&mut reader)?;
    let struct_layouts = Vec::get(&mut reader)?;
    let row_entries = Vec::get(&mut reader)?;
    let state = Option::get(&mut reader)?;
    let params = Vec::get(&mut reader)?;
    let passes = Vec::get(&mut reader)?;
    reader.finish()?;
    // Validate the entire manifest before attaching either section of bulk data.
    let tables = read_bulk::<TableDecl>(tables, packed_rows)?;
    let modules = read_bulk::<ModuleBlob>(modules, packed_modules)?;
    if shader_flags(&modules) != flags {
        return Err(BundleError::Encoding);
    }
    let rows: BTreeMap<_, _> = row_entries.iter().copied().collect();
    if !rows
        .iter()
        .eq(row_entries.iter().map(|(kind, table)| (kind, table)))
    {
        return Err(BundleError::Encoding);
    }
    Ok(Bundle {
        resources,
        modules,
        tables,
        struct_layouts,
        rows,
        state,
        params,
        passes,
    })
}

fn read_bulk<T: SplitRecord>(entries: Vec<T::Fields>, bytes: &[u8]) -> Result<Vec<T>, BundleError> {
    let mut reader = Reader::new(bytes);
    if length(&mut reader)? != entries.len() {
        return Err(BundleError::Encoding);
    }
    let out = entries
        .into_iter()
        .map(|fields| Ok(T::join(fields, get_bytes(&mut reader)?.into())))
        .collect::<Result<_, _>>()?;
    reader.finish()?;
    Ok(out)
}

/// Validate the portable shader payload without creating a device or a pipeline.
pub fn validate_wgsl(bundle: &Bundle) -> Result<(), BundleError> {
    use naga::valid::{Capabilities, ValidationFlags, Validator};

    // Bundle validation checks portable shader structure independently of device support.
    let capabilities = Capabilities::default()
        | Capabilities::SUBGROUP
        | Capabilities::SHADER_FLOAT32_ATOMIC
        | Capabilities::RAY_QUERY
        | Capabilities::SHADER_FLOAT16;
    for module in &bundle.modules {
        if module.target != ShaderTarget::Wgsl {
            return Err(BundleError::Shader(
                module.id,
                "expected portable WGSL".into(),
            ));
        }
        let shader = |reason: String| BundleError::Shader(module.id, reason);
        let source = std::str::from_utf8(&module.bytes).map_err(|e| shader(e.to_string()))?;
        crate::wgsl::validate(source, &mut Validator::new(ValidationFlags::all(), capabilities))
        .map_err(|e| shader(e.emit_to_string(source)))?;
    }
    Ok(())
}

macro_rules! ids {
    ($($ty:ident),+) => {$(
        impl Wire for $ty {
            fn put(&self, out: &mut Vec<u8>) { self.0.put(out); }
            fn get(input: &mut Reader<'_>) -> wire::Result<Self> { Ok(Self(Wire::get(input)?)) }
        }
    )+};
}

trait SplitRecord: Sized {
    type Fields;

    fn put_metadata(&self, out: &mut Vec<u8>);
    fn get_metadata(input: &mut Reader<'_>) -> wire::Result<Self::Fields>;
    fn join(fields: Self::Fields, bytes: Box<[u8]>) -> Self;
}

fn put_metadata(records: &[impl SplitRecord], out: &mut Vec<u8>) {
    (records.len() as u64).put(out);
    for record in records {
        record.put_metadata(out);
    }
}

fn get_metadata<T: SplitRecord>(input: &mut Reader<'_>) -> wire::Result<Vec<T::Fields>> {
    (0..length(input)?)
        .map(|_| T::get_metadata(input))
        .collect()
}

macro_rules! decode_record {
    ($input:ident nested, $record:expr) => {
        $input.nested(|$input| $record)
    };
    ($input:ident, $record:expr) => {
        $record
    };
}

macro_rules! record {
    ($(#[$attr:meta])* $ty:ident {
        $($(#[$doc:meta])* $field:ident: $field_ty:ty),+ $(,)?
    } / $bulk:ident: $blob:ty) => {
        impl SplitRecord for $ty {
            type Fields = ($($field_ty,)+);

            fn put_metadata(&self, out: &mut Vec<u8>) {
                $(self.$field.put(out);)+
            }

            fn get_metadata(input: &mut Reader<'_>) -> wire::Result<Self::Fields> {
                Ok(($(<$field_ty>::get(input)?,)+))
            }

            fn join(($($field,)+): Self::Fields, $bulk: $blob) -> Self {
                Self { $($field),+, $bulk }
            }
        }
    };
    ($(#[$attr:meta])* $ty:ident {
        $($(#[$doc:meta])* $field:ident: $field_ty:ty),+ $(,)?
    } $($guard:ident)?) => {
        impl Wire for $ty {
            fn put(&self, out: &mut Vec<u8>) {
                let Self { $($field),+ } = self;
                $($field.put(out);)+
            }

            fn get(input: &mut Reader<'_>) -> wire::Result<Self> {
                decode_record!(input $($guard)?, Ok(Self { $($field: Wire::get(input)?),+ }))
            }
        }
    };
}

loft_ir::bundle_schema!(ids, record, sum);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_wgsl_with_loft_capabilities() {
        for source in [
            "@compute @workgroup_size(1) fn main() { let sum = subgroupAdd(1u); }",
            "enable f16; @compute @workgroup_size(1) fn main() { let value = 1.0h; }",
            "@group(0) @binding(0) var<storage, read_write> sum: atomic<f32>;
             @compute @workgroup_size(1) fn main() { atomicAdd(&sum, 1.0); }",
            "enable wgpu_ray_query;
             @group(0) @binding(0) var scene: acceleration_structure;
             @compute @workgroup_size(1) fn main() {
                 var query: ray_query;
                 rayQueryInitialize(&query, scene, RayDesc(0u, 255u, 0.0, 1.0, vec3f(0.0), vec3f(0.0, 0.0, 1.0)));
                 let hit = rayQueryProceed(&query);
             }",
        ] {
            let bundle = Bundle {
                modules: vec![ModuleBlob {
                    id: ModuleId(0),
                    hash: *blake3::hash(source.as_bytes()).as_bytes(),
                    target: ShaderTarget::Wgsl,
                    bytes: source.as_bytes().into(),
                }],
                ..Bundle::default()
            };
            validate_wgsl(&bundle).unwrap_or_else(|err| panic!("{source}\n{err:?}"));
        }
    }
}
