//! Compiler-free LOFTB001 IO. Shader validation and device setup are separate from file decoding.
use loft_ir::*;
use std::{collections::BTreeMap, fmt};
pub(super) mod wire;
use wire::{Reader, Wire, fields, get_bytes, length, put_bytes, variants};

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

struct Manifest {
    modules: Vec<(ModuleId, ([u8; 32], ShaderTarget))>,
    tables: Vec<TableMeta>,
    struct_layouts: Vec<StructLayout>,
    rows: Vec<(KindId, TableId)>,
    state: Option<StateStoreDecl>,
    params: Vec<ParamMeta>,
    passes: Vec<PassDecl>,
}

struct TableMeta {
    id: TableId,
    format: ElemFormat,
    lifetime: TableLifetime,
    stride_bytes: u32,
    capacity: u32,
}

pub fn encode(bundle: &Bundle) -> Vec<u8> {
    let mut manifest = Vec::new();
    // Keep the bulk bytes in their own sections and leave declaration order intact.
    (bundle.modules.len() as u64).put(&mut manifest);
    for module in &bundle.modules {
        (module.id, (module.hash, module.target)).put(&mut manifest);
    }
    (bundle.tables.len() as u64).put(&mut manifest);
    for table in &bundle.tables {
        TableMeta {
            id: table.id,
            format: table.format,
            lifetime: table.lifetime,
            stride_bytes: table.stride_bytes,
            capacity: table.capacity,
        }
        .put(&mut manifest);
    }
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
    let flags = if bundle
        .modules
        .iter()
        .any(|m| m.target == ShaderTarget::Wgsl)
    {
        HAS_WGSL
    } else {
        0
    };
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
    for (slot, tag) in sections[..count].iter_mut().zip(TAGS.into_iter().chain([ASSETS])) {
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
        if resources.is_empty() { return Err(BundleError::Encoding); }
        resources
    } else {
        Resources::default()
    };
    let mut reader = Reader::new(meta);
    let manifest = Manifest::get(&mut reader)?;
    reader.finish()?;
    let tables = read_bulk(manifest.tables, packed_rows, |t, initial| TableDecl {
        id: t.id,
        format: t.format,
        lifetime: t.lifetime,
        stride_bytes: t.stride_bytes,
        capacity: t.capacity,
        initial,
    })?;
    let modules = read_bulk(
        manifest.modules,
        packed_modules,
        |(id, (hash, target)), bytes| ModuleBlob {
            id,
            hash,
            target,
            bytes,
        },
    )?;
    let has_wgsl = modules.iter().any(|m| m.target == ShaderTarget::Wgsl);
    if has_wgsl != (flags == HAS_WGSL) {
        return Err(BundleError::Encoding);
    }
    let rows: BTreeMap<_, _> = manifest.rows.iter().copied().collect();
    if !rows
        .iter()
        .eq(manifest.rows.iter().map(|(kind, table)| (kind, table)))
    {
        return Err(BundleError::Encoding);
    }
    Ok(Bundle {
        resources,
        modules,
        tables,
        struct_layouts: manifest.struct_layouts,
        rows,
        state: manifest.state,
        params: manifest.params,
        passes: manifest.passes,
    })
}

fn read_bulk<T, U>(
    entries: Vec<T>,
    bytes: &[u8],
    join: impl Fn(T, Box<[u8]>) -> U,
) -> Result<Vec<U>, BundleError> {
    let mut reader = Reader::new(bytes);
    if length(&mut reader)? != entries.len() {
        return Err(BundleError::Encoding);
    }
    let out = entries
        .into_iter()
        .map(|entry| Ok(join(entry, get_bytes(&mut reader)?.into())))
        .collect::<Result<_, _>>()?;
    reader.finish()?;
    Ok(out)
}

/// Validate the portable shader payload without creating a device or a pipeline.
pub fn validate_wgsl(bundle: &Bundle) -> Result<(), BundleError> {
    for module in &bundle.modules {
        if module.target != ShaderTarget::Wgsl {
            return Err(BundleError::Shader(
                module.id,
                "expected portable WGSL".into(),
            ));
        }
        let shader = |reason: String| BundleError::Shader(module.id, reason);
        let source = std::str::from_utf8(&module.bytes).map_err(|e| shader(e.to_string()))?;
        let ir =
            naga::front::wgsl::parse_str(source).map_err(|e| shader(e.emit_to_string(source)))?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
        .validate(&ir)
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
ids!(ModuleId, TableId, PassId, KindId, ParamId);

impl<T: Wire> Wire for Box<[T]> {
    fn put(&self, out: &mut Vec<u8>) {
        (self.len() as u64).put(out);
        for value in self {
            value.put(out);
        }
    }
    fn get(input: &mut Reader<'_>) -> wire::Result<Self> {
        Ok(Vec::get(input)?.into_boxed_slice())
    }
}

fields!(Manifest {
    modules,
    tables,
    struct_layouts,
    rows,
    state,
    params,
    passes
});
fields!(TableMeta {
    id,
    format,
    lifetime,
    stride_bytes,
    capacity
});
fields!(StructLayout {
    stride_bytes,
    offsets
});
fields!(StateStoreDecl {
    table,
    layout_hash,
    checkpoint_slots
});
fields!(ParamMeta {
    id,
    name,
    ty,
    offset_bytes,
    default,
    range
});
impl Wire for PassDecl {
    fn put(&self, out: &mut Vec<u8>) {
        self.id.put(out);
        self.wave.put(out);
        self.queue.put(out);
        self.bindings.put(out);
        self.run.put(out);
    }

    fn get(input: &mut Reader<'_>) -> wire::Result<Self> {
        input.nested(|input| {
            Ok(Self {
                id: Wire::get(input)?,
                wave: Wire::get(input)?,
                queue: Wire::get(input)?,
                bindings: Wire::get(input)?,
                run: Wire::get(input)?,
            })
        })
    }
}
fields!(PassBinding {
    group,
    slot,
    resource,
    access
});
fields!(IndirectDraw {
    module,
    vertex_entry,
    fragment_entry,
    args,
    offset_bytes,
    vertex,
    index
});
fields!(TextureDecl { table, format, extent, samples, clear });
fields!(FrameData { main, variants, time_offset });
fields!(InputSlot { kind, offset_words, words });
variants!(InputKind { 0 => Level, 1 => Event, 2 => Packet });
fields!(Resources { inputs, textures, comparison_samplers, resets, cull_front, frame });
variants!(ImageFormat { 0 => Rg32Uint, 1 => Rgba16Float, 2 => Depth32Float });
fields!(TargetDesc { table });
fields!(DepthDesc { table, write });
variants!(ShaderTarget { 0 => NagaIr, 1 => Wgsl, 2 => Msl, 3 => Spirv, 4 => Ptx, 5 => Wasm });
variants!(ElemFormat {
    0 => F32, 1 => F16, 2 => Bf16, 3 => F64, 4 => U32, 5 => I32, 6 => U16, 7 => I16,
    8 => U8, 9 => I8, 10 => PackedU4, 11 => PackedI4, 12 => PackedU2, 13 => Fp8E4M3,
    14 => Fp8E5M2, 15 => Struct(index),
});
variants!(TableLifetime { 0 => Static, 1 => DynamicArena { max_rows }, 2 => Transient, 3 => PingPong, 4 => External });
variants!(IrTy { 0 => F32, 1 => Vec2, 2 => Vec3, 3 => Vec4, 4 => U32, 5 => Bool });
variants!(QueueClass { 0 => Graphics, 1 => AsyncCompute, 2 => Transfer });
variants!(BindAccess { 0 => ReadStorage, 1 => ReadWriteStorage, 2 => SampledTex, 3 => StorageTex, 4 => BindlessTexArray, 5 => Sampler });
variants!(DispatchExtent { 0 => Fixed(groups), 1 => FromRowCount { table, workgroup_x }, 2 => Indirect { buf, offset_bytes } });
variants!(PassRun {
    0 => Compute { module, entry, dispatch },
    1 => ComputeFusedTicks { module, entry, max_ticks, instances },
    2 => RenderBundle { draws, color_targets, depth, msaa },
    3 => PassLoop { max_iters, cond_buf, body },
    4 => GenerateMips { texture },
});

#[cfg(test)]
#[test]
fn container_rejects_corruption() {
    let module = b"@compute @workgroup_size(1) fn main() {}";
    let bundle = Bundle {
        modules: vec![ModuleBlob {
            id: ModuleId(4),
            hash: *blake3::hash(module).as_bytes(),
            target: ShaderTarget::Wgsl,
            bytes: module.as_slice().into(),
        }],
        tables: vec![TableDecl {
            id: TableId(7),
            format: ElemFormat::F32,
            lifetime: TableLifetime::DynamicArena { max_rows: 5 },
            stride_bytes: 4,
            capacity: 2,
            initial: [0x00, 0x00, 0x00, 0x80].as_slice().into(),
        }],
        rows: BTreeMap::from([(KindId(3), TableId(7))]),
        passes: vec![PassDecl {
            id: PassId(2),
            wave: 1,
            queue: QueueClass::Graphics,
            bindings: Box::new([]),
            run: PassRun::Compute {
                module: ModuleId(4),
                entry: "main".into(),
                dispatch: DispatchExtent::Fixed([1, 1, 1]),
            },
        }],
        ..Bundle::default()
    };
    let mut encoded = encode(&bundle);
    let decoded = decode(&encoded).unwrap();
    validate_wgsl(&decoded).unwrap();
    assert_eq!(encode(&decoded), encoded);
    assert_eq!(decoded.tables[0].initial.as_ref(), &[0, 0, 0, 0x80]);
    assert_eq!(
        decode(&encoded[..encoded.len() - 1]).unwrap_err(),
        BundleError::Truncated
    );
    *encoded.last_mut().unwrap() ^= 1;
    assert_eq!(decode(&encoded).unwrap_err(), BundleError::Hash);
}
