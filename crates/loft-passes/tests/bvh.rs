use loft_abi::LeafRef;
use loft_emit::BVH_WGSL as WGSL;
use loft_eval::bvh::{Bvh2, BvhError, Ray};
use loft_passes::program::Instance;
use std::convert::Infallible;

fn ray(o: [f32; 3], d: [f32; 3], far: f32) -> Ray {
    Ray::new(o, d, 0.0, far).unwrap()
}

#[test]
fn deterministic_topology_and_atomic_refit() {
    let mut rows: Vec<_> = (0..65)
        .map(|row| {
            let x = row as f32 * 2.0;
            Instance {
                lo: [x, 0.0, 0.0],
                hi: [x + 1.0, 1.0, 1.0],
                kind: 3,
                row,
            }
        })
        .collect();
    let mut tree = Bvh2::build(&rows).unwrap();
    let topology = |tree: &Bvh2| {
        tree.nodes()
            .iter()
            .map(|n| (n.left_or_first, n.count))
            .collect::<Vec<_>>()
    };
    let original = topology(&tree);
    let leaves = tree.leaves().to_vec();
    rows.reverse();
    let reverse = Bvh2::build(&rows).unwrap();
    assert_eq!(topology(&reverse), original);
    assert_eq!(reverse.leaves(), leaves);
    let query = ray([0.5, 0.5, 2.0], [0.0, 0.0, -1.0], 2.0);
    let run = |tree: &Bvh2| {
        tree.trace(query, |leaf, _| {
            Ok::<_, Infallible>((leaf.row_id == 0).then_some(1.0))
        })
        .unwrap()
    };
    assert_eq!(
        run(&tree),
        Some((
            LeafRef {
                kind_id: 3,
                row_id: 0
            },
            1.0
        ))
    );
    assert_eq!(tree.refit(&rows[..64]), Err(BvhError::ChangedMembership));
    assert_eq!(run(&tree).unwrap().1, 1.0);
    for row in &mut rows {
        row.lo[0] += 5.0;
        row.hi[0] += 5.0;
    }
    tree.refit(&rows).unwrap();
    assert_eq!(tree.nodes()[0].lo, [5.0, 0.0, 0.0]);
    assert_eq!(tree.nodes()[0].hi, [134.0, 1.0, 1.0]);
    assert_eq!(topology(&tree), original);
    assert_eq!(tree.leaves(), leaves);
    assert_eq!(run(&tree), None);
    // Parallel to the X and Y slabs, on their shared edge.
    let edge = ray([5.0, 1.0, 2.0], [0.0, 0.0, -1.0], 1.0);
    let mut visits = 0;
    assert!(
        tree.occluded(edge, |_, _| {
            visits += 1;
            Ok::<_, Infallible>(Some(1.0))
        })
        .unwrap()
    );
    assert_eq!(visits, 1);
    assert!(Bvh2::build(&[]).unwrap().nodes().is_empty());
}

#[test]
fn traversal_wgsl_validates() {
    let source = format!(
        "{WGSL}\n{}",
        r#"
@group(0) @binding(0) var<storage, read> loft_bvh_nodes: array<LoftBvhNode>;
@group(0) @binding(1) var<storage, read> loft_bvh_leaves: array<LoftLeafRef>;
@group(0) @binding(2) var<storage, read_write> result: array<LoftBvhHit>;
fn loft_bvh_field(leaf: LoftLeafRef, p: vec3<f32>) -> f32 {
  return length(p) - (1.0 + f32(leaf.row_id));
}
fn loft_bvh_intersect(leaf: LoftLeafRef, ray: LoftRay) -> f32 {
  switch leaf.kind_id {
    case 0u: { return -ray.o.z / ray.d.z; }
    default: { return loft_bvh_march(leaf, ray, 1.0); }
  }
}
@compute @workgroup_size(1) fn sample() {
  let r = LoftRay(vec3<f32>(0.0, 0.0, 3.0), vec3<f32>(0.0, 0.0, -1.0), 0.0, 4.0);
  let n = arrayLength(&loft_bvh_nodes);
  result[0u] = loft_bvh_trace(r, n, false);
  result[1u] = loft_bvh_trace(r, n, true);
}
"#
    );
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    );
    validator.validate(&module).unwrap();
    for (name, size) in [("LoftBvhNode", 32), ("LoftLeafRef", 8)] {
        let ty = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some(name))
            .unwrap()
            .1;
        let naga::TypeInner::Struct { span, .. } = ty.inner else {
            panic!("expected wire struct")
        };
        assert_eq!(span, size);
    }
}
