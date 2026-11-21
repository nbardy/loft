use super::Bvh2;
use loft_check::{paths::PathTrace, program::{Instance, KindTable}};
use loft_ir::{Code, Diag, Res, Span};

pub fn pack_world(kinds: &[KindTable], instances: &[Instance], data: &PathTrace, words: &mut Vec<f32>) -> Res<()> {
    let overflow = || Diag::new(Code::E602, Span::NONE, "tiled World arena exceeds its address range", "use fewer geometry rows");
    let address = |len: usize| u32::try_from(len).map_err(|_| overflow());
    let base = address(words.len())?;
    words[0] = f32::from_bits(base);
    words.resize(words.len() + 1 + 2 * kinds.len(), 0.0);
    for (i, kind) in kinds.iter().enumerate() {
        words[base as usize + 1 + 2 * i] = f32::from_bits(address(words.len())? - base);
        words[base as usize + 2 + 2 * i] = f32::from_bits(kind.stride);
        words.extend_from_slice(&kind.rows);
    }
    words[base as usize] = f32::from_bits(address(words.len())? - base);
    let media_count = address(data.media.len())?;
    let mut descriptors = vec![address(data.worlds.len())?, media_count];
    let mut nodes = Vec::new();
    let mut leaves = Vec::new();
    for world in &data.worlds {
        let members = instances.iter().filter(|i| world.kinds.contains(&i.kind)).copied().collect::<Vec<_>>();
        let tree = Bvh2::build(&members).map_err(|error| Diag::new(
            Code::E602, Span::NONE, format!("invalid tiled World bounds: {error:?}"), "check the drawable bounds",
        ))?;
        let node_base = address(nodes.len() / 8)?;
        let leaf_base = address(leaves.len() / 2)?;
        let root = if tree.nodes().is_empty() { 0 } else { node_base.checked_add(1).ok_or_else(overflow)? };
        descriptors.extend([root, if world.media { media_count } else { 0 }]);
        for node in tree.nodes() {
            let offset = if node.count == 0 { node_base } else { leaf_base };
            nodes.extend(node.lo.map(f32::to_bits));
            nodes.push(node.left_or_first.checked_add(offset).ok_or_else(overflow)?);
            nodes.extend(node.hi.map(f32::to_bits));
            nodes.push(node.count);
        }
        leaves.extend(tree.leaves().iter().flat_map(|leaf| [leaf.kind_id, leaf.row_id]));
    }
    words.extend(descriptors.into_iter().map(f32::from_bits));
    words.extend_from_slice(data.media.as_flattened().as_flattened());
    words[1] = f32::from_bits(address(words.len())?);
    words.extend(nodes.into_iter().map(f32::from_bits));
    words[2] = f32::from_bits(address(words.len())?);
    words.extend(leaves.into_iter().map(f32::from_bits));
    words[3] = f32::from_bits(address(words.len())?);
    words[4] = f32::from_bits(address(data.lamps.len())?);
    words.extend_from_slice(data.lamps.as_flattened().as_flattened());
    // The native TLAS primitive order is the compiled instance order.
    // World membership is per kind, so the same bounds serve all World views.
    words[5] = f32::from_bits(address(words.len())?);
    let mask_words = data.worlds.len().div_ceil(32);
    words[6] = f32::from_bits(address(mask_words + 2)?);
    for instance in instances {
        words.extend([instance.kind, instance.row].map(f32::from_bits));
        for worlds in data.worlds.chunks(32) {
            let mut mask = 0u32;
            for (i, world) in worlds.iter().enumerate() {
                if world.kinds.contains(&instance.kind) { mask |= 1 << i; }
            }
            words.push(f32::from_bits(mask));
        }
    }
    address(words.len())?;

    Ok(())
}
