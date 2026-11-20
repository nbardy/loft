// CPU-only corpus samples. Link with the prebuilt loft-passes rlib.
use loft_passes::source::Graph;
use loft_passes::program::{Draw, Instance, Program};
use loft_passes::{Compiled, CpuQuery};
use std::path::Path;

const UP: [f32; 3] = [0.0, 1.0, 0.0];
const FRONT: [f32; 3] = [0.0, 0.0, 1.0];

fn compile(scene: &str, view: u32) -> (Program, Compiled) {
    let path = Path::new("corpus/l5c").join(scene).with_extension("loft");
    let sources = Graph::file(&path, "").unwrap();
    let (program, _, twin) = loft_passes::compile(&sources, &[("view".into(), f64::from(view))])
        .unwrap_or_else(|diag| panic!("{}", diag.render(&sources)));
    assert!(!program.modules.is_empty(), "{scene}");
    (program, twin)
}

fn query(twin: &Compiled, row: &Instance, point: [f32; 3], dir: [f32; 3]) -> CpuQuery {
    let q = twin
        .cpu_query(row.kind as usize, row.row as usize, point, dir)
        .unwrap();
    assert!(q.trace.is_finite() && q.visible.is_finite());
    assert!(
        q.material
            .into_iter()
            .all(|value| value.is_finite() && value >= 0.0)
    );
    q
}

fn near(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} vs {expected}"
    );
}

fn truth(program: &Program, twin: &Compiled) {
    for row in &program.instances {
        let kind = &program.kinds[row.kind as usize];
        assert!(
            (0..3).all(|axis| {
                row.lo[axis].is_finite() && row.hi[axis].is_finite() && row.lo[axis] < row.hi[axis]
            }),
            "{} row {}: {:?}..{:?}",
            kind.name,
            row.row,
            row.lo,
            row.hi
        );
        match kind.draw {
            Draw::Surface { .. } => {
                for (u, v) in [(0.13, 0.17), (0.5, 0.41), (0.84, 0.73)] {
                    let q = query(twin, row, [u, v, 0.0], FRONT);
                    for axis in 0..3 {
                        assert!(
                            q.surf_p[axis] >= row.lo[axis] && q.surf_p[axis] <= row.hi[axis],
                            "{} row {} axis {axis}: {} outside {:?}..{:?}",
                            kind.name,
                            row.row,
                            q.surf_p[axis],
                            row.lo,
                            row.hi,
                        );
                    }
                    let past = query(twin, row, [u - 0.001, v, 0.0], FRONT).surf_p;
                    let next = query(twin, row, [u + 0.001, v, 0.0], FRONT).surf_p;
                    let left = query(twin, row, [u, v - 0.001, 0.0], FRONT).surf_p;
                    let right = query(twin, row, [u, v + 0.001, 0.0], FRONT).surf_p;
                    let du: [f32; 3] = std::array::from_fn(|i| next[i] - past[i]);
                    let dv: [f32; 3] = std::array::from_fn(|i| right[i] - left[i]);
                    let cross = [
                        du[1] * dv[2] - du[2] * dv[1],
                        du[2] * dv[0] - du[0] * dv[2],
                        du[0] * dv[1] - du[1] * dv[0],
                    ];
                    let length = cross.into_iter().map(|v| v * v).sum::<f32>().sqrt();
                    let cosine = (0..3).map(|i| cross[i] / length * q.surf_n[i]).sum::<f32>();
                    assert!(
                        cosine.clamp(-1.0, 1.0).acos() <= 0.01,
                        "{} row {}: cos {cosine}",
                        kind.name,
                        row.row
                    );
                }
            }
            Draw::Analytic | Draw::Traced => {
                let center: [f32; 3] = std::array::from_fn(|i| (row.lo[i] + row.hi[i]) * 0.5);
                let center_trace = query(twin, row, center, FRONT).trace;
                let l = kind.lipschitz[row.row as usize];
                assert!(l.is_finite() && l > 0.0);
                for axis in 0..3 {
                    let mut offset = center;
                    offset[axis] += (row.hi[axis] - center[axis]) * 0.6;
                    let distance = offset[axis] - center[axis];
                    let trace = query(twin, row, offset, FRONT).trace;
                    assert!((trace - center_trace).abs() <= l * distance + 2e-6);
                    offset[axis] = row.hi[axis] + 0.025;
                    assert!(query(twin, row, offset, FRONT).trace > 0.0);
                }
            }
        }
        if kind.name == "Ground" {
            near(
                query(twin, row, [0.0, 3.0, 0.0], [0.0, -1.0, 0.0]).visible,
                3.0,
                1e-6,
            );
            near(query(twin, row, [0.0; 3], UP).trace, 0.0, 1e-6);
        }
    }
}

#[test]
fn bead() {
    for view in 0..=1 {
        let (program, twin) = compile("bead", view);
        truth(&program, &twin);
        assert_eq!(program.instances.len(), 2);
        let bead = program
            .instances
            .iter()
            .find(|row| program.kinds[row.kind as usize].name == "Bead")
            .unwrap();
        let axial = query(&twin, bead, [0.0, 4.0, 0.0], [0.0, -1.0, 0.0]);
        assert!(axial.visible < 0.0, "axial ray hit the bore");
        near(
            query(&twin, bead, [0.0, 1.0, -3.0], FRONT).visible,
            2.0,
            1e-6,
        );
        near(query(&twin, bead, [0.0, 1.0, -1.0], FRONT).trace, 0.0, 1e-6);
        near(query(&twin, bead, [0.0, 1.0, 0.0], FRONT).trace, 0.35, 1e-6);
        let wall = [0.6, -0.8, 0.0];
        near(
            query(&twin, bead, [0.0, 2.2, 0.0], wall).visible,
            0.35 / 0.6,
            1e-6,
        );
    }
}

fn lantern(scene: &str, count: usize) {
    for view in 0..=1 {
        let (program, twin) = compile(scene, view);
        truth(&program, &twin);
        assert_eq!(program.instances.len(), count);
        let paper = program
            .instances
            .iter()
            .filter(|row| program.kinds[row.kind as usize].name == "Paper");
        assert_eq!(paper.clone().count(), 15);
        for row in paper {
            let band = row.row as f32;
            let m = (((band - 0.5) / 14.0).max(0.0) + ((band + 0.5) / 14.0).min(1.0)) * 0.5;
            let [x, y, z] = query(&twin, row, [0.5, 0.41, 0.0], FRONT).surf_p;
            let expected = (0.16 + 0.59 * (std::f32::consts::PI * m).sin())
                * (0.95 + 0.05 * (14.0 * std::f32::consts::PI * m).cos().abs());
            near(x.hypot(z), expected, 1e-5);
            near(y, 0.04 + 1.8 * m, 1e-5);
        }
    }
}

#[test]
fn lantern_with_ground() {
    lantern("first__lantern", 16);
}

#[test]
fn lantern_without_ground() {
    lantern("first__lantern_nothing", 15);
}
