use super::*;

/// Native render passes apply one disjoint hardware scissor per indirect argument.
pub const SIDE: u32 = 32;

pub(super) struct TileList {
    pub rows: TableId,
    pub args: TableId,
}

pub(super) fn prepare(
    b: &mut Builder, p: &Program, size: (u32, u32), kinds: &[(TableId, TableId)],
    visible: TableId, args: TableId, arg_words: u32, hdr: ImageFormat,
) -> Result<BTreeMap<usize, TileList>> {
    let mut lists = BTreeMap::new();
    let grid = [size.0.div_ceil(SIDE), size.1.div_ceil(SIDE), 1];
    let tiles = u64::from(grid[0]) * u64::from(grid[1]);
    // Keep the tile-by-row table below 64 MiB per kind and 128 MiB per scene.
    // An oversized problem keeps its original indirect draw and row list.
    let mut remaining = 32 * 1024 * 1024u64;
    let mut module = None;
    for (&id, balls) in &p.segments {
        let n = p.kinds[id as usize].n_rows;
        if balls.len() != n as usize { return Err("segment endpoint count differs from the row table".into()); }
        let words = tiles.saturating_mul(u64::from(n));
        if words > remaining.min(16 * 1024 * 1024) || n == 0 { continue; }
        remaining -= words;
        let index = match module {
            Some(index) => index,
            None => {
                let source = include_str!("segment_tiles.wgsl").replace("TILE_SIDE", &SIDE.to_string());
                let ir = naga::front::wgsl::parse_str(&source).map_err(|e| e.emit_to_string(&source))?;
                let (code, reflected) = shader(ir, "segment tiles", None, arg_words, hdr)?;
                let index = b.bundle.modules.len();
                b.bindings.push(reflected);
                b.bundle.modules.push(ModuleBlob {
                    id: ModuleId(index as u32), hash: *blake3::hash(code.as_bytes()).as_bytes(),
                    target: ShaderTarget::Wgsl, bytes: code.into_bytes().into_boxed_slice(),
                });
                module = Some(index);
                index
            }
        };
        let endpoints = b.words(4, balls.iter().flatten().flatten().copied().map(f32::to_bits));
        let rects = b.transient(16, n);
        let rows = b.transient(4, words as u32);
        let tile_args = b.transient(16, tiles as u32);
        let resources = Resources::from([
            ((1,0),endpoints), ((1,1),visible), ((1,2),kinds[id as usize].1), ((1,3),args),
            ((1,4),rects), ((1,5),rows), ((1,6),tile_args),
        ]);
        b.compute(index, "project", DispatchExtent::Fixed([n.div_ceil(64),1,1]), &resources)?;
        b.compute(index, "gather", DispatchExtent::Fixed(grid), &resources)?;
        lists.insert(id as usize, TileList { rows, args: tile_args });
    }
    Ok(lists)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_and_gather_validate_with_mixed_indexed_indirect_stride() {
        let code = include_str!("segment_tiles.wgsl").replace("TILE_SIDE", &SIDE.to_string());
        let ir = naga::front::wgsl::parse_str(&code).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&code)));
        let (_, bindings) = shader(ir, "segment tiles", None, 5, ImageFormat::Rgba16Float).unwrap();
        assert!(bindings.contains_key(&("project".into(), ShaderStage::Compute)));
        assert!(bindings.contains_key(&("gather".into(), ShaderStage::Compute)));
    }
}
