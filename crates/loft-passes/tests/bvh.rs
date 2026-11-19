use loft_abi::LeafRef;
use loft_ir::Sources;
use loft_passes::bvh::{Bvh2, BvhError, CpuWorld, Ray, WGSL};
use loft_passes::program::{Draw, Instance, Program};
use loft_passes::{Compiled, compile};
use std::convert::Infallible;

fn compile_scene(scene: &str, overrides: &[(&str, f64)]) -> (Program, Compiled) {
    let mut src = Sources::default();
    for (name, prelude) in [
        ("core", include_str!("../../../lib/core.loft")),
        ("bsdf", include_str!("../../../lib/bsdf.loft")),
        ("light", include_str!("../../../lib/light.loft")),
        ("analytic", include_str!("../../../lib/analytic.loft")),
        ("sdf", include_str!("../../../lib/sdf.loft")),
        ("world", include_str!("../../../lib/world.loft")),
    ] {
        src.add(name, prelude);
    }
    src.add("scene", scene);
    let overrides: Vec<_> = overrides
        .iter()
        .map(|&(name, n)| (name.to_owned(), n))
        .collect();
    let (program, _, compiled) =
        compile(&src, &overrides).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    (program, compiled)
}

fn ray(o: [f32; 3], d: [f32; 3], far: f32) -> Ray {
    Ray::new(o, d, 0.0, far).unwrap()
}

fn vacuum(_: [f32; 3], _: [f32; 3]) -> loft_ir::Res<[f32; 3]> {
    Ok([1.0; 3])
}

#[test]
fn washer_oracle_ids_and_dual_shadow_masks() {
    let source = include_str!("bvh/washer.loft");
    let (solid, solid_twin) = compile_scene(source, &[("washer_inner", 0.0)]);
    let solid_prog = solid_twin.prog();
    let mut shadow_solid = CpuWorld::new(&solid_prog, &solid_twin, &solid, &[0, 1]).unwrap();
    for (inner, expected_count, edited_pick) in [(0.5, 9_664, 1), (0.25, 12_080, 0)] {
        let (program, twin) = compile_scene(source, &[("washer_inner", inner)]);
        assert_eq!(program.kinds[0].name, "Washer");
        assert_eq!(program.kinds[1].name, "Card");
        assert_eq!(program.kinds[0].draw, Draw::Analytic);
        let prog = twin.prog();
        let mut primary = CpuWorld::new(&prog, &twin, &program, &[0, 1]).unwrap();
        let mut masks = [0, 0, 0];
        let pixel = |i: usize| (i as f32 + 0.5) / 64.0 - 4.0;
        for y in 0..512 {
            let py = pixel(y);
            for x in 0..512 {
                let px = pixel(x);
                let hit = primary
                    .trace(ray([px, py, 3.0], [0.0, 0.0, -1.0], 5.0))
                    .unwrap()
                    .expect("the card fills the grid");
                let rr = px * px + py * py;
                let ring = (inner as f32 * inner as f32..=1.0).contains(&rr);
                // Legacy oracle object IDs 7 and 9 map to the two kind IDs, both at row zero.
                assert_eq!(
                    hit.leaf,
                    LeafRef {
                        kind_id: u32::from(!ring),
                        row_id: 0
                    },
                    "primary {x},{y}"
                );
                assert_eq!(hit.t, if ring { 2.0 } else { 3.0 });
                masks[0] += usize::from(ring);

                let a = [px, py, 0.0];
                let b = [px + 4.0, py, 2.0];
                let sx = pixel(x + 128);
                let shadow_rr = sx * sx + py * py;
                let ring_shadow = (inner as f32 * inner as f32..=1.0).contains(&shadow_rr);
                let blocked = primary.visible(a, b, vacuum).unwrap() == [0.0; 3];
                let solid = shadow_solid.visible(a, b, vacuum).unwrap() == [0.0; 3];
                assert_eq!(blocked, ring_shadow, "annular shadow {x},{y}");
                assert_eq!(solid, shadow_rr <= 1.0, "solid shadow {x},{y}");
                masks[1] += usize::from(blocked);
                masks[2] += usize::from(solid);
            }
        }
        assert_eq!(masks, [expected_count, expected_count, 12_892]);
        for (y, kind, t) in [
            (0.0, 1, 3.0),
            (0.375, edited_pick, 2.0 + edited_pick as f32),
        ] {
            for (z, offset) in [(3.0, 0.0), (2.0, 1.0)] {
                let hit = primary
                    .trace(ray([0.0, y, z], [0.0, 0.0, -1.0], 10.0))
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    hit.leaf,
                    LeafRef {
                        kind_id: kind,
                        row_id: 0
                    }
                );
                assert!((hit.t - (t - offset)).abs() <= 1e-4);
            }
        }
        eprintln!(
            "washer inner {inner}: primary {}, annular shadow {}, solid shadow {}",
            masks[0], masks[1], masks[2]
        );
    }
}

#[test]
fn bead_bore_rays_through_traced_kind() {
    let (program, twin) = compile_scene(include_str!("bvh/bead.loft"), &[]);
    assert_eq!(program.kinds[0].draw, Draw::Traced);
    let prog = twin.prog();
    let mut world = CpuWorld::new(&prog, &twin, &program, &[0]).unwrap();
    let down = [0.0, -1.0, 0.0];
    let east = [1.0, 0.0, 0.0];
    assert!(
        world
            .trace(ray([0.0, 4.0, 0.0], down, 10.0))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        world
            .visible([0.0, 4.0, 0.0], [0.0, -1.0, 0.0], |_, _| Ok([
                0.5, 0.8, 0.9
            ]))
            .unwrap(),
        [0.5, 0.8, 0.9]
    );
    for (o, d, t, n) in [
        ([0.6, 4.0, 0.0], down, 2.2, [0.6, 0.8, 0.0]),
        ([0.0, 1.0, 0.0], east, 0.35, [-1.0, 0.0, 0.0]),
        ([0.6, 1.0, 0.0], east, 0.4, east),
    ] {
        let r = ray(o, d, 10.0);
        let hit = world.trace(r).unwrap().unwrap();
        assert!((hit.t - t).abs() <= 1e-5, "{o:?} {hit:?}");
        for i in 0..3 {
            // Normal is a finite difference of the f32 field.
            assert!((hit.n[i] - n[i]).abs() <= 1e-3, "{o:?} {hit:?}");
        }
        let end = r.at(t + 0.1);
        assert_eq!(
            world
                .visible(o, end, |_, _| panic!("blocked rays skip medium"))
                .unwrap(),
            [0.0; 3]
        );
        assert_eq!(world.visible(o, r.at(t * 0.5), vacuum).unwrap(), [1.0; 3]);
        eprintln!("bead {o:?}: t={}, normal={:?}", hit.t, hit.n);
    }
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

#[test]
fn coral_shrine_has_analytic_branches_and_two_lantern_snapshots() {
    let scene = include_str!("../../../engines/coral_shrine.loft");
    let (primary, primary_twin) = compile_scene(scene, &[]);
    let (shadow, shadow_twin) = compile_scene(scene, &[("shrine_solid", 1.0)]);
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    );
    for (a, b) in primary.modules.iter().zip(&shadow.modules) {
        assert_eq!(a.wgsl, b.wgsl, "shader {}", a.name);
        validator.validate(&a.naga).unwrap();
    }
    assert_eq!(primary.modules.len(), shadow.modules.len());
    assert_eq!(
        (primary.kinds[2].draw, primary.kinds[2].n_rows),
        (Draw::Analytic, 3069)
    );
    let p = primary_twin.prog();
    let s = shadow_twin.prog();
    let mut world = CpuWorld::new(&p, &primary_twin, &primary, &[0, 1, 2]).unwrap();
    assert_eq!(
        (world.bvh().nodes().len(), world.bvh().leaves().len()),
        (2047, 3071)
    );
    let hit = world
        .trace(ray([-0.6, 0.4, 2.13], [1.0, 0.0, 0.0], 0.9))
        .unwrap()
        .unwrap();
    assert_eq!(
        hit.leaf,
        LeafRef {
            kind_id: 2,
            row_id: 0
        }
    );
    assert!(hit.n[0] < -0.9, "{hit:?}");

    let mut carved = CpuWorld::new(&p, &primary_twin, &primary, &[1]).unwrap();
    let mut solid = CpuWorld::new(&s, &shadow_twin, &shadow, &[1]).unwrap();
    let (a, b) = ([0.0, 0.05, 0.0], [0.0, 2.2, 0.0]);
    assert_eq!(carved.visible(a, b, vacuum).unwrap(), [1.0; 3]);
    assert_eq!(solid.visible(a, b, vacuum).unwrap(), [0.0; 3]);
}
