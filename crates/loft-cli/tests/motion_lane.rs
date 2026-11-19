use loft_ir::*;
use loft_passes::fusion::{self, RESULTS, ROWS};
use loft_runner::Runner;
use std::{collections::BTreeMap, sync::Arc};

const PASS: PassId = PassId(0);
const EVENTS: TableId = TableId(3);
const BOSS: &str = include_str!("motion_lane/boss_events.loft");
const PIN: &str = include_str!("../../../tests/fixtures/pin-drift.loft");

fn sources(source: &str) -> Sources {
    let lib = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../lib"));
    let mut prelude: Vec<_> = std::fs::read_dir(lib)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "loft"))
        .collect();
    prelude.sort();
    let mut src = Sources::default();
    for path in prelude {
        src.add(
            path.display().to_string(),
            std::fs::read_to_string(&path).unwrap(),
        );
    }
    src.add("motion-proof.loft", source);
    src
}

fn fixture(source: &str, overrides: &[(String, f64)]) -> Bundle {
    let src = sources(source);
    let program = loft_passes::compile(&src, overrides)
        .unwrap_or_else(|d| panic!("{}", d.render(&src)))
        .0;
    assert_eq!(program.dispatches.len(), 1);
    fusion::motion_bundle(&program, &program.dispatches[0], 3)
}

fn tables(bundle: &Bundle) -> BTreeMap<TableId, Vec<u8>> {
    bundle
        .tables
        .iter()
        .map(|decl| {
            let mut data = decl.initial.to_vec();
            data.resize((decl.stride_bytes * decl.capacity) as usize, 0);
            if decl.id == EVENTS {
                data[4..8].copy_from_slice(&511u32.to_le_bytes());
            }
            (decl.id, data)
        })
        .collect()
}

fn step(b: &Bundle, tables: &mut BTreeMap<TableId, Vec<u8>>, from: u32, to: u32) {
    let count = b.tables.iter().find(|t| t.id == ROWS).unwrap().capacity;
    Runner::step_bundle_cpu(
        b,
        PASS,
        tables,
        InstanceId(0),
        0..count,
        fusion::advance(from, to, 0).unwrap(),
    )
    .unwrap();
}

fn compare_grouping(b: &Bundle) -> BTreeMap<TableId, Vec<u8>> {
    let mut baseline = tables(b);
    step(b, &mut baseline, 0, 0);
    let snapshot =
        |data: &BTreeMap<TableId, Vec<u8>>| (data[&ROWS].clone(), data.get(&RESULTS).cloned());
    let mut trace = vec![snapshot(&baseline)];
    for tick in 1..=480 {
        step(b, &mut baseline, tick - 1, tick);
        trace.push(snapshot(&baseline));
    }
    assert_ne!(trace[0], trace[480]);
    for interval in [4, 2, 1] {
        let mut live = tables(b);
        let mut t = 0;
        while t < 480 {
            let end = if t == 120 { t + 6 } else { t + interval }.min(480);
            for n in t + 1..end {
                let mut inside = live.clone();
                step(b, &mut inside, t, n);
                assert_eq!(
                    snapshot(&inside),
                    trace[n as usize],
                    "interval {interval}, tick {n}"
                );
            }
            step(b, &mut live, t, end);
            assert_eq!(
                snapshot(&live),
                trace[end as usize],
                "interval {interval}, tick {end}"
            );
            t = end;
        }
        assert_eq!(live.get(&EVENTS), baseline.get(&EVENTS));
    }
    baseline
}

#[test]
fn motion_cpu_tick_grouping_and_event_payloads() {
    let b = fixture(BOSS, &[]);
    let baseline = compare_grouping(&b);
    let out = &baseline[&EVENTS];
    let words: Vec<_> = out[..80]
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(&words[..4], &[2, 511, 0, 0]);
    assert_eq!(&words[4..8], &[fusion::event_tag("ClawHit"), 0, 1, 3]);
    assert_eq!(&words[8..12], &[0.5_f32, 1., 0.25, 2.].map(f32::to_bits));
    assert_eq!(&words[12..16], &[fusion::event_tag("FootSlip"), 0, 3, 4]);
    assert_eq!(&words[16..20], &[0.25_f32, 0., 0.5, 0.02].map(f32::to_bits));
}

#[test]
fn motion_cpu_verdict_catches_fake_foot_20mm() {
    for (drift, expected) in [(0., 1), (0.02, 2)] {
        let b = fixture(PIN, &[("drift_m".into(), drift)]);
        let mut data = tables(&b);
        step(&b, &mut data, 0, 0);
        let words: Vec<_> = data[&RESULTS]
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(&words[..6], &[0, 0, 0, 0, expected, 0]);
        assert_eq!(
            f32::from_bits(words[6]),
            if expected == 1 { 0.00005 } else { 0.02 }
        );
        assert_ne!(words[7], 0);
    }
}

fn washer(x: u32, y: u32, r: i32) -> (u32, u32) {
    let dx = 2 * x as i32 + 1 - 512;
    let dy = 2 * y as i32 + 1 - 512;
    if (r * r..=128 * 128).contains(&(dx * dx + dy * dy)) {
        (7, 3)
    } else {
        (1, 2)
    }
}

#[test]
fn motion_cpu_washer_oracle() {
    let ir = naga::front::wgsl::parse_str(include_str!("motion_lane/washer_pick.wgsl")).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&ir)
    .unwrap();
    for (r, expected) in [(64, 9664), (32, 12080)] {
        let count = (0..512)
            .flat_map(|y| (0..512).map(move |x| washer(x, y, r)))
            .filter(|&pair| pair == (7, 3))
            .count();
        assert_eq!(count, expected);
    }
}

fn gpu() -> (Runner, loft_cli::native::Gpu) {
    let g = loft_cli::native::gpu().unwrap();
    let runner = Runner::attach(
        Arc::new(g.device.clone()),
        Arc::new(g.queue.clone()),
        HostCaps {
            depth_convention: DepthConvention::StandardZeroToOne,
            msaa_samples: 1,
            features: GpuFeatures::empty(),
        },
    );
    (runner, g)
}

#[test]
fn motion_gpu_alternates_cpu_and_gpu_ticks() {
    let b = fixture(BOSS, &[]);
    let (mut runner, g) = gpu();
    let id = runner.load_bundle(&b).unwrap();
    let mut baseline = tables(&b);
    let mut events = vec![];
    for t in 1..=10 {
        step(&b, &mut baseline, t - 1, t);
        if t % 2 == 0 {
            let data = runner.step_pass_cpu(id, PASS, 0..2, 1).unwrap();
            assert_eq!(data, baseline[&ROWS]);
        } else {
            let mut enc = g.device.create_command_encoder(&Default::default());
            runner.advance_into(id, &mut enc, t, 3).unwrap();
            g.queue.submit([enc.finish()]);
            g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(
                runner.step_pass_cpu(id, PASS, 0..2, 0).unwrap(),
                baseline[&ROWS]
            );
            let (found, drops) = runner.drain_events(id).unwrap();
            events.extend(found);
            assert_eq!(drops, 0);
        }
    }
    assert_eq!(
        events
            .iter()
            .map(|e| (e.tag, e.instance_id, e.site_id, e.tick, e.payload))
            .collect::<Vec<_>>(),
        [
            (fusion::event_tag("ClawHit"), 0, 1, 3, [0.5, 1., 0.25, 2.]),
            (
                fusion::event_tag("FootSlip"),
                0,
                3,
                4,
                [0.25, 0., 0.5, 0.02]
            ),
        ]
    );
    runner.restore_checkpoint(id, 3).unwrap();
    let mut checkpoint = tables(&b);
    step(&b, &mut checkpoint, 0, 3);
    assert_eq!(
        runner.step_pass_cpu(id, PASS, 0..2, 0).unwrap(),
        checkpoint[&ROWS]
    );
}

#[test]
fn motion_gpu_washer_picks_sample_zero() {
    use wgpu::util::DeviceExt;
    use wgpu::{TextureFormat as F, TextureUsages as U};
    let (mut r, g) = gpu();
    let d = &g.device;
    let pixels = wgpu::Extent3d {
        width: 512,
        height: 512,
        depth_or_array_layers: 1,
    };
    let texture = |format| {
        d.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: pixels,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
            view_formats: &[],
        })
    };
    let vis = texture(F::Rgba16Uint);
    let depth = texture(F::Depth32Float);
    let id = r.load_bundle(&Bundle::default()).unwrap();
    r.set_pick_source(
        id,
        &vis,
        &depth,
        [
            [4., 0., 0., 0.],
            [0., 4., 0., 0.],
            [0., 0., -4., 0.],
            [0., 0., 3., 1.],
        ],
    )
    .unwrap();

    let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(include_str!("motion_lane/washer_pick.wgsl").into()),
    });
    let pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("cover"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("draw"),
            targets: &[Some(F::Rgba16Uint.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: F::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    for inner in [64i32, 32] {
        let radius = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: &(inner * inner).to_le_bytes(),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: radius.as_entire_binding(),
            }],
        });
        let v = vis.create_view(&Default::default());
        let z = depth.create_view(&Default::default());
        let mut enc = d.create_command_encoder(&Default::default());
        {
            let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &v,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &z,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            p.set_pipeline(&pipeline);
            p.set_bind_group(0, &bind, &[]);
            p.draw(0..3, 0..1);
        }
        g.queue.submit([enc.finish()]);
        for px in [
            [256, 256],
            [271, 256],
            [272, 256],
            [287, 256],
            [288, 256],
            [319, 256],
            [320, 256],
            [224, 288],
        ] {
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            r.pick_async(
                id,
                px,
                Box::new(move |p| {
                    let _ = tx.send(p);
                }),
            )
            .unwrap();
            d.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let picked = rx.recv().unwrap();
            let sync = r.pick_sync(id, px).unwrap();
            let oracle = washer(px[0], px[1], inner);
            assert!(picked.hit && sync.hit);
            assert_eq!((picked.kind_id, picked.row_id), oracle);
            assert_eq!((sync.kind_id, sync.row_id), oracle);
            assert_eq!(picked.world_pos, sync.world_pos);
        }
    }
}

#[test]
fn motion_cpu_outbox_shader_for_generic_compute_entries() {
    let mut wgsl = fusion::event_wgsl(2, 4, "60u");
    wgsl.push_str(
        r#"
        @compute @workgroup_size(1) fn kernel_tick() { emit_event(7u, 2u, 4u, vec4f(1.)); }
        @compute @workgroup_size(1) fn solver_tick() { emit_event(8u, 2u, 3u, vec4f(2.)); }
    "#,
    );
    let ir = naga::front::wgsl::parse_str(&wgsl).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&ir)
    .unwrap();
    let (storage, _) = ir
        .global_variables
        .iter()
        .find(|(_, v)| v.name.as_deref() == Some("loft_outbox"))
        .unwrap();
    for i in 0..2 {
        assert!(!info.get_entry_point(i)[storage].is_empty());
    }
}

#[test]
fn motion_cpu_rejects_overlapping_fields() {
    let source = BOSS
        .replace("h.{toe}", "h.{phase}")
        .replace("h.toe =", "h.phase =");
    let src = sources(&source);
    assert_eq!(
        loft_passes::compile(&src, &[]).err().unwrap().code,
        Code::E502
    );
}

fn peer_sources(name: &str) -> Sources {
    let root = std::env::var_os("LOFT_MOTION_FIXTURES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")));
    let scene = root.join("scenes").join(format!("{name}.loft"));
    let mut src = sources(&std::fs::read_to_string(scene).expect("supply the l23c scene fixtures"));
    if std::env::var_os("LOFT_MOTION_FIXTURES").is_some() {
        for file in ["lib/motion.loft", "engines/roam.loft", "engines/act.loft"] {
            if !src.files.iter().any(|(p, _)| p.ends_with(file)) {
                src.add(file, std::fs::read_to_string(root.join(file)).unwrap());
            }
        }
    }
    src
}

fn compile_peer(src: &Sources, overrides: &[(String, f64)]) -> Bundle {
    let p = loft_passes::compile(src, overrides)
        .unwrap_or_else(|d| panic!("{}", d.render(src)))
        .0;
    assert_eq!(p.dispatches.len(), 1);
    fusion::motion_bundle(&p, &p.dispatches[0], 3)
}

#[test]
fn motion_wave_cpu_crab_grouping_and_mutant() {
    let mut src = peer_sources("crab");
    let clean = compile_peer(&src, &[]);
    let scored = compare_grouping(&clean);
    assert_eq!(scored[&RESULTS][16..20], 1u32.to_le_bytes());
    let script = &mut src
        .files
        .iter_mut()
        .find(|(p, _)| p == "motion-proof.loft")
        .unwrap()
        .1;
    assert!(script.contains("toe_bias: f = 0.0"));
    *script = script.replace("toe_bias: f = 0.0", "toe_bias: f = 0.020");
    let shifted = compile_peer(&src, &[]);
    let mut data = tables(&shifted);
    step(&shifted, &mut data, 0, 0);
    assert_eq!(data[&RESULTS][16..20], 2u32.to_le_bytes());
}

fn boss_hit_one() -> Bundle {
    compile_peer(&peer_sources("boss"), &[])
}

const BOSS_PAYLOADS: [(u32, [f32; 4]); 2] =
    [(1, [-1800., -900., 0., 420.]), (0, [0., 0.06, 0., 0.35])];

#[test]
fn motion_wave_cpu_boss_hit_one() {
    let bundle = boss_hit_one();
    let mut data = tables(&bundle);
    step(&bundle, &mut data, 0, 60);
    let events = &data[&EVENTS];
    assert_eq!(&events[0..4], &2u32.to_le_bytes());
    assert_eq!(&events[8..12], &0u32.to_le_bytes());
    for (i, (site, payload)) in BOSS_PAYLOADS.into_iter().enumerate() {
        let tag = fusion::event_tag(["ClawHit", "FootSlip"][i]);
        let expected = [
            tag,
            0,
            site,
            60,
            payload[0].to_bits(),
            payload[1].to_bits(),
            payload[2].to_bits(),
            payload[3].to_bits(),
        ];
        assert_eq!(
            &events[16 + i * 32..48 + i * 32],
            expected.map(u32::to_le_bytes).as_flattened()
        );
    }
}

// Runs on the combined wave where l23c supplies the scene and its prelude.
#[test]
fn motion_gpu_boss_hit_one() {
    let bundle = boss_hit_one();
    let (mut runner, g) = gpu();
    let id = runner.load_bundle(&bundle).unwrap();
    for tick in [60, 61, 62] {
        let mut encoder = g.device.create_command_encoder(&Default::default());
        runner.advance_into(id, &mut encoder, tick, 0).unwrap();
        g.queue.submit([encoder.finish()]);
    }
    g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let (emitted, dropped) = runner.drain_events(id).unwrap();
    assert_eq!(dropped, 0);
    assert_eq!(emitted.len(), 2);
    for (i, (event, (site, payload))) in emitted.into_iter().zip(BOSS_PAYLOADS).enumerate() {
        let tag = fusion::event_tag(["ClawHit", "FootSlip"][i]);
        assert_eq!(
            (
                event.tag,
                event.instance_id,
                event.site_id,
                event.tick,
                event.payload
            ),
            (tag, 0, site, 60, payload)
        );
    }
}
