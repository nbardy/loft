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
        // Check every pixel in the washer's bounding square through the actual async API.
        // Keep at most one scanline's staging buffers and callbacks in flight.
        let mut ring_count = 0;
        for y in 192..320 {
            let (tx, rx) = std::sync::mpsc::channel();
            for x in 192..320 {
                let tx = tx.clone();
                r.pick_async(
                    id,
                    [x, y],
                    Box::new(move |v| {
                        let v = v.unwrap();
                        tx.send((x, (v.kind_id, v.row_id))).unwrap();
                    }),
                )
                .unwrap();
            }
            drop(tx);
            d.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let mut returned = std::collections::BTreeSet::new();
            for (x, ids) in rx {
                assert!(returned.insert(x));
                assert_eq!(ids, washer(x, y, inner));
                ring_count += usize::from(ids == (7, 3));
            }
            assert_eq!(returned.len(), 128);
        }
        assert_eq!(ring_count, if inner == 64 { 9664 } else { 12080 });
        println!("Washer inner {inner}: {ring_count} exact ring IDs in 16384 async picks");
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
            let picked = rx.recv().unwrap().unwrap();
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

#[test]
fn profile_json_counts_executed_fixture() {
    let src = sources(include_str!("../../../engines/capsules.loft"));
    let mut p = loft_passes::compile(&src, &[]).unwrap().0;
    let fixture = include_str!("../../../tests/fixtures/telemetry.wgsl");
    p.modules[p.classify].naga = naga::front::wgsl::parse_str(fixture).unwrap();
    p.modules[p.classify].name = "fixture".into();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.sexp");
    let json = dir.path().join("profile.json");
    std::fs::write(&path, loft_passes::sexp::write(&p).unwrap()).unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_loft"))
        .arg("profile")
        .arg(path)
        .args(["--frames", "3", "--size", "8x8", "--json"])
        .arg(&json)
        .status()
        .unwrap();
    assert!(status.success());
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(json).unwrap()).unwrap();
    let m = report["modules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["module_id"] == "fixture")
        .unwrap();
    for (key, expected) in [
        ("alu_ops", 2),
        ("transcendentals", 1),
        ("storage_loads", 1),
        ("storage_stores", 1),
        ("workgroup_bytes", 64),
    ] {
        assert_eq!(m[key], expected, "{key}");
    }
    assert_eq!(report["gpu_frame_ms"].as_array().unwrap().len(), 3);
    println!("profile fixture {m}");
}

fn mapped_bytes(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Vec<u8> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    buffer.map_async(wgpu::MapMode::Read, .., move |r| tx.send(r).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let bytes = buffer.get_mapped_range(..).unwrap().to_vec();
    buffer.unmap();
    bytes
}

#[test]
fn crab_gpu_grouped_frames_match_every_tick() {
    let bundle = compile_peer(&peer_sources("crab"), &[]);
    let g = loft_cli::native::gpu().unwrap();
    let fresh = || {
        let mut runner = Runner::attach(
            Arc::new(g.device.clone()),
            Arc::new(g.queue.clone()),
            HostCaps {
                depth_convention: DepthConvention::StandardZeroToOne,
                msaa_samples: 1,
                features: GpuFeatures::empty(),
            },
        );
        let id = runner.load_bundle(&bundle).unwrap();
        (runner, id)
    };
    let trace = |interval: u32| {
        let (mut r, id) = fresh();
        let stride = [ROWS, RESULTS]
            .map(|t| r.export_buffer(id, t).unwrap().size())
            .iter()
            .sum::<u64>();
        let dest = g.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("481 state and verdict records"),
            size: 481 * stride,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let capture = |r: &mut Runner, t: u32, keep: u32| {
            let mut enc = g.device.create_command_encoder(&Default::default());
            r.advance_into(id, &mut enc, t, keep).unwrap();
            let mut offset = u64::from(t) * stride;
            for table in [ROWS, RESULTS] {
                let buffer = r.export_buffer(id, table).unwrap();
                enc.copy_buffer_to_buffer(&buffer, 0, &dest, offset, Some(buffer.size()));
                offset += buffer.size();
            }
            g.queue.submit([enc.finish()]);
        };
        capture(&mut r, 0, 0);
        let mut start = 0;
        while start < 480 {
            let end = (start + if start == 120 { 6 } else { interval }).min(480);
            for t in start + 1..end {
                capture(&mut r, t, 0);
                // Each within-frame prefix starts at the same retained boundary.
                if start == 0 {
                    r = fresh().0;
                } else {
                    r.restore_checkpoint(id, start).unwrap();
                }
            }
            capture(&mut r, end, end);
            start = end;
        }
        (mapped_bytes(&g.device, &dest), stride as usize)
    };
    let (reference, stride) = trace(1);
    assert_ne!(&reference[..stride], &reference[480 * stride..]);
    for interval in [2, 4] {
        let (sample, _) = trace(interval);
        let mismatched: Vec<_> = sample
            .chunks_exact(stride)
            .zip(reference.chunks_exact(stride))
            .enumerate()
            .filter_map(|(tick, (a, b))| (a != b).then_some(tick))
            .collect();
        assert!(
            mismatched.is_empty(),
            "{} differing ticks: {:?}",
            mismatched.len(),
            mismatched.first()
        );
    }
    println!("Crab GPU state and verdicts: 0/481 differing ticks; 30/60/120 Hz and 6-tick stall");
}

#[test]
fn crab_gpu_thousand_row_motion_budget() {
    use wgpu::util::DeviceExt;
    let mut b = compile_peer(&peer_sources("crab"), &[]);
    let g = loft_cli::native::gpu().unwrap();
    assert!(g.timestamps);
    let control = g.triangle_control_ms((1280, 800)).unwrap();
    let d = &g.device;
    let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(std::str::from_utf8(&b.modules[0].bytes).unwrap().into()),
    });
    let p = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("advance"),
        compilation_options: Default::default(),
        cache: None,
    });
    let rows = b.tables.iter_mut().find(|t| t.id == ROWS).unwrap();
    let row_data = rows.initial.repeat(1000);
    let result_size = b
        .tables
        .iter()
        .find(|t| t.id == RESULTS)
        .unwrap()
        .stride_bytes as usize
        * 1000;
    let make = |bytes: &[u8]| {
        d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        })
    };
    let args = make(&[0; 24]);
    let rows = make(&row_data);
    let results = make(&vec![0; result_size]);
    let bind = |i: u32, entries: &[wgpu::BindGroupEntry]| {
        d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &p.get_bind_group_layout(i),
            entries,
        })
    };
    let g0 = bind(0, &[]);
    let g1 = bind(
        1,
        &[wgpu::BindGroupEntry {
            binding: 0,
            resource: args.as_entire_binding(),
        }],
    );
    let g2 = bind(
        2,
        &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: rows.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: results.as_entire_binding(),
            },
        ],
    );
    const N: u32 = 330;
    let queries = d.create_query_set(&wgpu::QuerySetDescriptor {
        label: None,
        ty: wgpu::QueryType::Timestamp,
        count: N * 2,
    });
    for i in 0..N {
        g.queue.write_buffer(
            &args,
            0,
            [i, i + 1, 0, 0, 1000, 0]
                .map(u32::to_le_bytes)
                .as_flattened(),
        );
        let mut enc = d.create_command_encoder(&Default::default());
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set: &queries,
                beginning_of_pass_write_index: Some(i * 2),
                end_of_pass_write_index: Some(i * 2 + 1),
            }),
        });
        pass.set_pipeline(&p);
        for (n, group) in [&g0, &g1, &g2].into_iter().enumerate() {
            pass.set_bind_group(n as u32, group, &[]);
        }
        pass.dispatch_workgroups(1000u32.div_ceil(64), 1, 1);
        drop(pass);
        g.queue.submit([enc.finish()]);
    }
    let buf = |usage| {
        d.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(N) * 16,
            usage,
            mapped_at_creation: false,
        })
    };
    let resolved = buf(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC);
    let read = buf(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
    let mut enc = d.create_command_encoder(&Default::default());
    enc.resolve_query_set(&queries, 0..2 * N, &resolved, 0);
    enc.copy_buffer_to_buffer(&resolved, 0, &read, 0, Some(read.size()));
    g.queue.submit([enc.finish()]);
    let mut ms = mapped_bytes(d, &read)
        .chunks_exact(16)
        .skip(30)
        .map(|b| {
            let (a, z) = b.split_at(8);
            let dt = u64::from_le_bytes(z.try_into().unwrap())
                - u64::from_le_bytes(a.try_into().unwrap());
            dt as f64 * f64::from(g.queue.get_timestamp_period()) / 1e6
        })
        .collect::<Vec<_>>();
    ms.sort_by(f64::total_cmp);
    let p95 = ms[284];
    println!("1000 Crab Scramble rows motion p95 {p95:.6} ms; control p50 {control:.6} ms");
    assert!(control > 0.35 || p95 <= 0.15);
}

#[test]
fn authored_gpu_heron_and_boss_verdicts() {
    let g = loft_cli::native::gpu().unwrap();
    for name in ["heron", "boss"] {
        let src = peer_sources(name);
        let p = loft_passes::compile(&src, &[]).unwrap().0;
        let step = &p.dispatches[0];
        let b = fusion::motion_bundle(&p, step, 0);
        let capture = |interval: u32| {
            let mut r = Runner::attach(
                Arc::new(g.device.clone()),
                Arc::new(g.queue.clone()),
                HostCaps {
                    depth_convention: DepthConvention::StandardZeroToOne,
                    msaa_samples: 1,
                    features: GpuFeatures::empty(),
                },
            );
            let id = r.load_bundle(&b).unwrap();
            let state = r.export_buffer(id, ROWS).unwrap();
            let records = r.export_buffer(id, RESULTS).unwrap();
            let stride = state.size() + records.size();
            let dst = g.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 481 * stride,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut ticks = vec![];
            let mut tick = 0;
            loop {
                let mut enc = g.device.create_command_encoder(&Default::default());
                r.advance_into(id, &mut enc, tick, 0).unwrap();
                let off = u64::from(tick) * stride;
                enc.copy_buffer_to_buffer(&state, 0, &dst, off, Some(state.size()));
                enc.copy_buffer_to_buffer(
                    &records,
                    0,
                    &dst,
                    off + state.size(),
                    Some(records.size()),
                );
                g.queue.submit([enc.finish()]);
                g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                assert_eq!(r.drain_events(id).unwrap().1, 0);
                ticks.push(tick as usize);
                if tick == 480 {
                    break;
                }
                tick = (tick
                    + if interval > 1 && tick == 120 {
                        6
                    } else {
                        interval
                    })
                .min(480);
            }
            (
                mapped_bytes(&g.device, &dst),
                stride as usize,
                state.size() as usize,
                ticks,
            )
        };
        let (bytes, stride, state_bytes, _) = capture(1);
        let field = |row: &[u8], name: &str| {
            let off = step.fields.iter().find(|(n, _)| n == name).unwrap().1 as usize * 4;
            f32::from_le_bytes(row[off..off + 4].try_into().unwrap())
        };
        let mut crouch = f32::INFINITY;
        let mut max_pitch = 0f32;
        let mut neck_error = 0f32;
        let mut windup = 0f32;
        let mut hit_ticks = vec![];
        let mut previous = 0.;
        for (tick, row) in bytes.chunks_exact(stride).enumerate() {
            for (i, rec) in row[state_bytes..].chunks_exact(32).enumerate() {
                assert_eq!(
                    u32::from_le_bytes(rec[16..20].try_into().unwrap()),
                    1,
                    "{name} {} tick {tick}",
                    step.checks[i]
                );
                if step.checks[i] == "heron_neck_length" {
                    neck_error = neck_error
                        .max(0.0002 - f32::from_le_bytes(rec[24..28].try_into().unwrap()));
                }
            }
            if name == "heron" {
                crouch = crouch.min(field(row, "hip_y"));
                max_pitch = max_pitch.max(field(row, "lean").abs().atan().to_degrees());
            } else {
                windup = windup.max(field(row, "windup"));
                let hits = field(row, "hits");
                if hits > previous {
                    hit_ticks.push(tick);
                    assert_eq!(field(row, "slipping"), 1.);
                }
                previous = hits;
            }
        }
        if name == "heron" {
            println!(
                "Heron GPU: min hip {crouch:.6} m, max pitch {max_pitch:.6} deg, max neck error {:.6} mm, all 481 forward gaze checks Held",
                1000. * neck_error
            );
            assert!((crouch - 0.59).abs() < 1e-5 && max_pitch <= 12. && neck_error <= 0.0002);
        } else {
            assert_eq!(hit_ticks, [60, 75]);
            assert!((windup - 0.69).abs() < 1e-6);
            let (grouped, _, _, ticks) = capture(4);
            for t in ticks {
                let span = t * stride..(t + 1) * stride;
                assert_eq!(&bytes[span.clone()], &grouped[span], "Boss at tick {t}");
            }
            println!(
                "Boss GPU: windup {windup:.6} m, hit ticks {hit_ticks:?}, grouped state and verdicts bit-identical at every frame boundary"
            );
        }
    }
}
