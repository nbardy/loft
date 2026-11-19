//! Measure the precompiled edit entry and its native promotion on the same device.
use super::*;
use crate::tests::{device_with_features, read};
use loft_abi::Tier0Opcode as Op;

const PIXELS: u32 = 400 * 350;
const ENTRY: &str = "loft_tier0_main";
const PASS: PassId = PassId(0);

fn words(radius: u16) -> Vec<u32> {
    [
        (Op::LoadRow, 0, 0, 0, 0),
        (Op::LoadConst, 1, 0, 0, radius),
        (Op::SdSphere, 2, 0, 1, 0),
        (Op::Ret, 0, 2, 0, 0),
    ]
    .map(|(op, d, a, b, i)| op.pack(d, a, b, i).unwrap())
    .into()
}

fn fixture(code: &str) -> Bundle {
    let points: Vec<_> = (0..PIXELS)
        .flat_map(|i| {
            [
                (i % 400) as f32 / 200. - 1.,
                (i / 400) as f32 / 175. - 1.,
                0.25,
                0.,
            ]
        })
        .flat_map(f32::to_le_bytes)
        .collect();
    let constants: Vec<_> = [0.35f32; 4]
        .into_iter()
        .chain([0.8; 4])
        .flat_map(f32::to_le_bytes)
        .collect();
    let mut bundle = Bundle::default();
    let mut bindings = vec![];
    for (id, group, slot, access, stride, count, initial) in [
        (0, 1, 0, BindAccess::ReadStorage, 4, 5, vec![]),
        (1, 1, 1, BindAccess::ReadStorage, 16, 2, constants),
        (2, 1, 2, BindAccess::ReadStorage, 16, PIXELS, points),
        (3, 1, 3, BindAccess::ReadStorage, 16, 1, vec![]),
        (4, 2, 0, BindAccess::ReadWriteStorage, 16, PIXELS, vec![]),
        (5, 3, 0, BindAccess::SampledTex, 4, 1, vec![]),
    ] {
        bundle.tables.push(TableDecl {
            id: TableId(id),
            format: ElemFormat::F32,
            stride_bytes: stride,
            capacity: count,
            initial: initial.into(),
            lifetime: if group == 3 {
                TableLifetime::External
            } else {
                TableLifetime::Static
            },
        });
        bindings.push(PassBinding {
            resource: TableId(id),
            group,
            slot,
            access,
        });
    }
    bundle.rows.insert(KindId(0), TableId(2));
    bundle.modules.push(ModuleBlob {
        id: ModuleId(0),
        target: ShaderTarget::Wgsl,
        bytes: code.as_bytes().into(),
        hash: *blake3::hash(code.as_bytes()).as_bytes(),
    });
    bundle.passes.push(PassDecl {
        id: PASS,
        wave: 0,
        queue: QueueClass::AsyncCompute,
        bindings: bindings.into(),
        run: PassRun::Compute {
            module: ModuleId(0),
            entry: ENTRY.into(),
            dispatch: DispatchExtent::Fixed([PIXELS.div_ceil(32), 1, 1]),
        },
    });
    bundle
}

fn frame(r: &mut Runner, id: InstanceId, view: &wgpu::TextureView) {
    let mut enc = r.device.create_command_encoder(&Default::default());
    r.encode_into(id, &mut enc, view, None, 0., 0.).unwrap();
    r.queue.submit([enc.finish()]);
}

#[test]
fn time_edit_and_compare_native_promotion() {
    use std::time::{Duration, Instant};
    let (mut r, view) = device_with_features(wgpu::Features::TIMESTAMP_QUERY);
    let code = concat!(include_str!("../../../loft-passes/src/tier0/compute.wgsl"), include_str!("../../../loft-passes/src/tier0.wgsl"));
    let display = "vec4f(clamp(0.5 - 2.0 * DIST.x, 0.0, 1.0))";
    let vm = code.replace(
        "= loft_tier0(gid.x);",
        &format!("= {};", display.replace("DIST", "loft_tier0(gid.x)")),
    );
    let specialized = vm.replace(
        "loft_tier0(gid.x)",
        "vec4f(length(tier_rows[gid.x].xyz) - tier_constants[1].x)",
    );
    let id = r.load_bundle(&fixture(&vm)).unwrap();
    let desc = wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    };
    r.declare_external_texture(id, TableId(5), &desc).unwrap();
    let tex = r.device.create_texture(&desc);
    r.bind_external_texture(id, TableId(5), Arc::new(tex))
        .unwrap();
    r.register_tier0(id, KindId(0), PASS).unwrap();
    let out = r.export_buffer(id, TableId(4)).unwrap();
    r.hot_swap_tape(id, KindId(0), &words(0)).unwrap();
    frame(&mut r, id, &view);
    let before = read(&r, &out);
    let edit = words(1);
    let start = Instant::now();
    r.hot_swap_tape(id, KindId(0), &edit).unwrap();
    let hot_ms = start.elapsed().as_secs_f64() * 1000.;
    frame(&mut r, id, &view);
    let interpreted = read(&r, &out);
    assert_ne!(before, interpreted);
    let vm_pipeline = r.instances[0].compute.pipeline(PASS).unwrap().clone();
    r.hot_swap_module(id, ModuleId(0), &specialized).unwrap();
    let start = Instant::now();
    while *r.instances[0].compute.pipeline(PASS).unwrap() == vm_pipeline {
        assert!(start.elapsed() < Duration::from_secs(20));
        frame(&mut r, id, &view);
        r.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }
    let promoted = read(&r, &out);
    let over = interpreted
        .chunks_exact(4)
        .zip(promoted.chunks_exact(4))
        .filter(|(a, b)| {
            a.iter()
                .zip(*b)
                .any(|(&x, &y)| (f32::from_bits(x) - f32::from_bits(y)).abs() > 1. / 255.)
        })
        .count();
    // Interleave both already-built pipelines with identical resources and workgroup counts.
    let cache = &r.instances[0].compute;
    let pass = &cache.passes[&PASS];
    const N: u32 = 120;
    let set = r.device.create_query_set(&wgpu::QuerySetDescriptor {
        label: None,
        ty: wgpu::QueryType::Timestamp,
        count: 4 * N,
    });
    for i in 0..N {
        let mut enc = r.device.create_command_encoder(&Default::default());
        for (j, pipeline) in [&vm_pipeline, &pass.pipeline].into_iter().enumerate() {
            let begin = 4 * i + 2 * j as u32;
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set: &set,
                    beginning_of_pass_write_index: Some(begin),
                    end_of_pass_write_index: Some(begin + 1),
                }),
            });
            cp.set_pipeline(pipeline);
            cp.set_bind_group(0, &cache.fixed, &[]);
            for (g, b) in pass.groups.iter().enumerate() {
                cp.set_bind_group(g as u32 + 1, &b.cached.as_ref().unwrap().1, &[]);
            }
            cp.dispatch_workgroups(PIXELS.div_ceil(32), 1, 1);
        }
        r.queue.submit([enc.finish()]);
    }
    let resolved = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 32 * u64::from(N),
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let mut enc = r.device.create_command_encoder(&Default::default());
    enc.resolve_query_set(&set, 0..4 * N, &resolved, 0);
    r.queue.submit([enc.finish()]);
    let data = read(&r, &resolved);
    let mut delta = vec![];
    for row in data.chunks_exact(8).skip(20) {
        let ts = |i: usize| u64::from(row[i * 2]) | (u64::from(row[i * 2 + 1]) << 32);
        let elapsed =
            |i| (ts(i + 1) - ts(i)) as f64 * f64::from(r.queue.get_timestamp_period()) / 1e6;
        delta.push(elapsed(0) - elapsed(2));
    }
    delta.sort_by(f64::total_cmp);
    println!(
        "Tier 0 bucket: hot upload {hot_ms:.6} ms, overhead p95 {:.6} ms, {over}/{PIXELS} pixels over 1/255",
        delta[94]
    );
    assert!(hot_ms <= 2. && delta[94] <= 0.5 && over <= PIXELS as usize / 1000);
}
