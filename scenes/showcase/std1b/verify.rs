// Focused observation using the same CPU queries and Metal twin as loft gate.
use loft_cli::native::{Gpu, Loaded, gpu};
use loft_passes::{Compiled, program::Program, source::Graph};
use std::path::Path;

type V = [f32; 3];
const Z: V = [0.0, 0.0, 1.0];
const X: V = [1.0, 0.0, 0.0];
const Y: V = [0.0, 1.0, 0.0];

fn dot(a: V, b: V) -> f32 {
    (0..3).map(|i| a[i] * b[i]).sum()
}

fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: V) -> V {
    a.map(|x| x / dot(a, a).sqrt())
}

fn near(label: &str, a: f32, b: f32, tol: f32) {
    assert!(
        a.is_finite() && b.is_finite() && (a - b).abs() <= tol,
        "{label}: {a} / {b}, tolerance {tol}"
    );
}

fn load(name: &str) -> (Program, Compiled) {
    let path = format!("scenes/showcase/std1b/{name}.loft");
    let graph = Graph::file(Path::new(&path), "").unwrap();
    let (program, _, compiled) =
        loft_passes::compile(&graph, &[]).unwrap_or_else(|d| panic!("{}", d.render(&graph)));
    (program, compiled)
}

struct Twin<'a> {
    gpu: &'a Gpu,
    loaded: Loaded,
    program: &'a Program,
    cpu: &'a Compiled,
}

impl<'a> Twin<'a> {
    fn new(gpu: &'a Gpu, program: &'a Program, cpu: &'a Compiled) -> Self {
        Self {
            gpu,
            program,
            cpu,
            loaded: Loaded::new(gpu, program, (64, 64), None).unwrap(),
        }
    }

    fn query(&self, name: &str, rays: &[(usize, V, V)]) -> Vec<([f32; 16], [f32; 16])> {
        let kind = self
            .program
            .kinds
            .iter()
            .position(|kind| kind.name == name)
            .unwrap();
        let input: Vec<f32> = rays
            .iter()
            .flat_map(|&(row, o, d)| {
                [
                    o[0],
                    o[1],
                    o[2],
                    f32::from_bits(row as u32),
                    d[0],
                    d[1],
                    d[2],
                    0.0,
                ]
            })
            .collect();
        let result = self.loaded.twin(self.gpu, kind, &input).unwrap();
        assert_eq!(result.len(), 16 * rays.len());
        rays.iter()
            .zip(result.chunks_exact(16))
            .map(|(&(row, p, rd), gpu)| {
                let q = self.cpu.cpu_query(kind, row, p, rd).unwrap();
                let cpu = [
                    q.trace,
                    q.visible,
                    0.,
                    0.,
                    q.material[0],
                    q.material[1],
                    q.material[2],
                    0.,
                    q.surf_p[0],
                    q.surf_p[1],
                    q.surf_p[2],
                    0.,
                    q.surf_n[0],
                    q.surf_n[1],
                    q.surf_n[2],
                    0.,
                ];
                let gpu: [f32; 16] = gpu.try_into().unwrap();
                assert!(
                    cpu.iter().chain(&gpu).all(|x| x.is_finite()),
                    "{name} row {row}, p={p:?}, rd={rd:?}, cpu={cpu:?}, gpu={gpu:?}"
                );
                (cpu, gpu)
            })
            .collect()
    }
}

fn cones(twin: &Twin) {
    let c = (1.0f32 - 0.25 * 0.25).sqrt();
    let cases = [
        ("big cap", 0, [0., -1.5, 0.], Y, 0.5, Some(-Y[1])),
        ("small cap", 0, [0., 4., 0.], [0., -1., 0.], 1.5, Some(1.)),
        (
            "side",
            0,
            [2., 1., 0.],
            [-1., 0., 0.],
            2. - 0.75 / c,
            Some(0.25),
        ),
        ("tangent", 1, [-2., 0.75, 1.], X, 2., Some(0.)),
        ("tangent miss", 1, [-2., 0.75, 1.0002], X, -1., None),
        ("inside exit", 1, [0.5, 1., 0.], X, 0.5, Some(0.)),
        (
            "coincident endpoints",
            2,
            [0., 0., 2.],
            [0., 0., -1.],
            1.25,
            Some(0.),
        ),
        (
            "contained first ball",
            3,
            [0., 0.2, 2.],
            [0., 0., -1.],
            1.3,
            Some(0.),
        ),
        (
            "cap/side join",
            0,
            [2., 0.25, 0.],
            [-1., 0., 0.],
            2. - c,
            Some(0.25),
        ),
        ("surface start", 1, [1., 1., 0.], X, 0., Some(0.)),
        (
            "remote camera",
            0,
            [60., 1., 0.],
            [-1., 0., 0.],
            60. - 0.75 / c,
            Some(0.25),
        ),
    ];
    let rays: Vec<_> = cases
        .iter()
        .map(|&(_, row, o, d, _, _)| (row, o, d))
        .collect();
    let hits = twin.query("Cone", &rays);
    let mut max_hit = 0.0f32;
    let mut max_surface = 0.0f32;
    let mut min_normal = 1.0f32;
    for ((label, row, o, d, expected, ny), (cpu, gpu)) in cases.into_iter().zip(hits) {
        near(label, cpu[1], expected, 1e-5);
        near(label, gpu[1], expected, 1e-5);
        max_hit = max_hit.max((cpu[1] - gpu[1]).abs());
        let Some(ny) = ny else {
            println!("cone {label}: miss");
            continue;
        };
        let hit: V = std::array::from_fn(|i| o[i] + expected * d[i]);
        let normal = match label {
            "tangent" | "coincident endpoints" | "contained first ball" => Z,
            _ => [(1. - ny * ny).sqrt(), ny, 0.],
        };
        let mut offsets = vec![(row, hit, Z)];
        for axis in 0..3 {
            for delta in [-0.001, 0.001] {
                let mut point = hit;
                point[axis] += delta;
                offsets.push((row, point, Z));
            }
        }
        let values = twin.query("Cone", &offsets);
        for (cpu_end, gpu_end) in &values {
            near(label, cpu_end[0], gpu_end[0], 1e-5);
        }
        max_surface = max_surface
            .max(values[0].0[0].abs())
            .max(values[0].1[0].abs());
        near(label, values[0].0[0], 0., 1e-5);
        near(label, values[0].1[0], 0., 1e-5);
        let cn = unit(std::array::from_fn(|i| {
            values[2 + 2 * i].0[0] - values[1 + 2 * i].0[0]
        }));
        let gn = unit(std::array::from_fn(|i| {
            values[2 + 2 * i].1[0] - values[1 + 2 * i].1[0]
        }));
        let oriented = dot(cn, normal).min(dot(gn, normal));
        min_normal = min_normal.min(oriented);
        assert!(
            oriented > 0.999_98,
            "{label}: normal {cn:?} / {gn:?}, expected {normal:?}"
        );
        println!(
            "cone {label}: t={:.8}/{:.8}, normal cosine={oriented:.8}",
            cpu[1], gpu[1]
        );
    }
    println!(
        "cone maximum twin hit delta={max_hit:.8} m, maximum absolute field at hit={max_surface:.8} m, minimum normal cosine={min_normal:.8}"
    );
}

fn sample_key(keys: [u32; 5]) -> f32 {
    let mut word = 0u32;
    for key in keys.into_iter().rev() {
        let state = (word ^ key)
            .wrapping_mul(747796405)
            .wrapping_add(2891336453);
        let mixed = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277803737);
        word = (mixed >> 22) ^ mixed;
    }
    (word >> 8) as f32 / 16777216.0
}

fn fields(twin: &Twin) {
    // A dyadic separation survives the octave offsets. At octave 8 it
    // spans 1/64 cell, versus 0.128 cell with a 1e-3 world-space step.
    let h = 1.0 / 8192.0;
    let points = [
        [-1. - h, -0.73, -0.51],
        [-1., -0.73, -0.51],
        [-1. + h, -0.73, -0.51],
        [-2.125, -3.925, 0.37],
        [-4., 2., 1.],
        [0.82, -0.42, 0.18],
    ];
    let mut worst = 0.0f32;
    for name in ["Lattice", "Cells", "Site"] {
        let rows = if name == "Lattice" { 2 } else { 1 };
        let queries: Vec<_> = (0..rows)
            .flat_map(|row| points.map(|p| (row, p, Z)))
            .collect();
        let values = twin.query(name, &queries);
        for (cpu, gpu) in &values {
            for i in 4..7 {
                near(name, cpu[i], gpu[i], 3e-5);
                worst = worst.max((cpu[i] - gpu[i]).abs());
            }
        }
        if name == "Lattice" {
            for edge in values.chunks_exact(points.len()) {
                for i in 4..7 {
                    let left = (edge[1].0[i] - edge[0].0[i]) / h;
                    let right = (edge[2].0[i] - edge[1].0[i]) / h;
                    near("lattice one-sided seam slopes", left, right, 0.12);
                    let gpu_left = (edge[1].1[i] - edge[0].1[i]) / h;
                    let gpu_right = (edge[2].1[i] - edge[1].1[i]) / h;
                    near("left seam CPU/GPU", left, gpu_left, 0.035);
                    near("right seam CPU/GPU", right, gpu_right, 0.035);
                    println!(
                        "lattice seam channel {i}: slopes CPU {left:.6}/{right:.6}, GPU {gpu_left:.6}/{gpu_right:.6}"
                    );
                }
            }
        } else if name == "Cells" {
            for (v, _) in values {
                assert!(
                    0. <= v[4] && v[4] <= v[5] && 0. <= v[6],
                    "invalid distance rank: {v:?}"
                );
            }
        }
    }
    println!("lattice/cellular maximum twin component delta = {worst:.8}");
    let marks = twin.query(
        "Marks",
        &[
            (0, [0., 0.25, 0.], Z),
            (0, [0.3, 0.25, 0.], Z),
            (0, [0.2, 0.9, 0.], Z),
        ],
    );
    for (cpu, gpu) in &marks {
        for i in 4..7 {
            near("mark twin", cpu[i], gpu[i], 1e-6);
        }
    }
    near("mark interior", marks[0].0[4], -0.25, 1e-6);
    near("zero width segment", marks[1].0[5], 0.3, 1e-6);
    near(
        "zero width and height point",
        marks[1].0[6],
        0.3f32.hypot(0.25),
        1e-6,
    );
    println!("signed mark, collapsed triangle/segment/point held");
}

fn sampling(twin: &Twin) {
    let queries = [(0, [0.; 3], Z), (1, [0.; 3], X), (1, [0.; 3], Y)];
    for (dimension, (cpu, gpu)) in [3, 4, 4].into_iter().zip(twin.query("Samples", &queries)) {
        let expected = sample_key([719, 373, 5, 2, dimension]);
        assert_eq!(
            (-cpu[0]).to_bits(),
            expected.to_bits(),
            "CPU counter {dimension}"
        );
        assert_eq!(
            (-gpu[0]).to_bits(),
            expected.to_bits(),
            "GPU counter {dimension}"
        );
        let radial = sample_key([719, 373, 5, 2, 3]);
        near("cosine half sphere", cpu[6], (1. - radial).sqrt(), 1e-6);
        near("cosine twin", cpu[6], gpu[6], 1e-6);
    }
    println!("display counter bits and cosine weights held on X, Y and Z");
}

fn surfaces(twin: &Twin, kinds: &[(&str, usize)]) {
    for &(kind, rows) in kinds {
        let mut max = 0.0f32;
        let mut min = 1.0f32;
        for row in 0..rows {
            let points = [
                (0.02, 0.08),
                (0.22, 0.49),
                (0.50, 0.24),
                (0.81, 0.83),
                (0.96, 0.66),
            ];
            let rays: Vec<_> = points
                .into_iter()
                .flat_map(|(u, v)| {
                    [
                        [u, v, 0.],
                        [u - 0.001, v, 0.],
                        [u + 0.001, v, 0.],
                        [u, v - 0.001, 0.],
                        [u, v + 0.001, 0.],
                    ]
                    .map(|uv| (row, uv, Z))
                })
                .collect();
            for q in twin.query(kind, &rays).chunks_exact(5) {
                let (cpu, gpu) = q[0];
                for i in [8, 9, 10, 12, 13, 14] {
                    let d = (cpu[i] - gpu[i]).abs();
                    max = max.max(d);
                    near(kind, cpu[i], gpu[i], 0.001);
                }
                let du = std::array::from_fn(|i| q[2].0[8 + i] - q[1].0[8 + i]);
                let dv = std::array::from_fn(|i| q[4].0[8 + i] - q[3].0[8 + i]);
                let oracle = unit(cross(du, dv));
                min = min
                    .min(dot(oracle, cpu[12..15].try_into().unwrap()))
                    .min(dot(oracle, gpu[12..15].try_into().unwrap()));
                assert!(
                    min > 0.9998,
                    "{kind} row {row}: normal disagrees with UV finite differences"
                );
            }
        }
        println!(
            "{kind}: max position/normal twin component delta {max:.8}, minimum finite-diff cosine {min:.8}"
        );
    }
}

fn frames(program: &Program, count: usize) {
    let mut min = 1.0f32;
    for f in program.luts[..count * 12].chunks_exact(12) {
        let t: V = f[3..6].try_into().unwrap();
        let n: V = f[6..9].try_into().unwrap();
        let b: V = f[9..12].try_into().unwrap();
        for axis in [t, n, b] {
            near("unit frame axis", dot(axis, axis), 1., 2e-5);
        }
        near("orthogonal", dot(t, n), 0., 2e-5);
        let det = dot(cross(t, n), b);
        near("orientation determinant", det, 1., 2e-5);
        min = min.min(det);
    }
    println!(
        "frames={count}, sampled bytes={}, minimum determinant={min:.8}",
        count * 48
    );
}

fn main() {
    let gpu = gpu().unwrap();
    println!("adapter {}", gpu.adapter);
    let (program, cpu) = load("probe");
    let twin = Twin::new(&gpu, &program, &cpu);
    if std::env::args().nth(1).as_deref() != Some("sampling-and-surfaces") {
        cones(&twin);
        fields(&twin);
    }
    sampling(&twin);
    frames(&program, 16);
    surfaces(&twin, &[("Ring", 1), ("FrameSample", 1), ("Lid", 2)]);
    let (shell, cpu) = load("shell_eye");
    frames(&shell, 96);
    surfaces(
        &Twin::new(&gpu, &shell, &cpu),
        &[("Carapace", 1), ("Mantle", 1), ("Eyelid", 4)],
    );
}
