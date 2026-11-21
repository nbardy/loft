use loft_cli::native::{self, Gpu, Loaded};
use loft_ir::{Code, DepthConvention, GpuFeatures, HostCaps};
use loft_passes::{Compiled, program::Program, source::Graph};
use loft_runner::Runner;
use std::{path::Path, sync::Arc};

fn scene(name: &str) -> Graph {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Graph::file(&root.join("scenes/showcase/cap1c").join(name), "").unwrap()
}

fn compile(source: &Graph, settings: &[(&str, f64)]) -> (Program, Compiled) {
    let overrides = settings
        .iter()
        .map(|(key, val)| (key.to_string(), *val))
        .collect::<Vec<_>>();
    let (program, _, compiled) =
        loft_passes::compile(source, &overrides).unwrap_or_else(|e| panic!("{}", e.render(source)));
    (program, compiled)
}

fn portable_bundle(p: &Program, c: &Compiled, size: (u32, u32)) -> loft_ir::Bundle {
    loft_passes::bundle::lower(p, c, size, |sun, alternate| {
        let light = alternate.map(|(lo, hi, side)| native::sun_camera(p.sun, lo, hi, side));
        native::frame_bytes(p.camera, p.sun, size, 0.0, sun, light)
    })
    .unwrap()
}

fn runner(g: &Gpu) -> Runner {
    Runner::attach(
        Arc::new(g.device.clone()),
        Arc::new(g.queue.clone()),
        HostCaps {
            depth_convention: DepthConvention::StandardZeroToOne,
            features: GpuFeatures::empty(),
            msaa_samples: 1,
        },
    )
}

fn target(g: &Gpu, size: (u32, u32)) -> wgpu::Texture {
    g.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: native::COLOR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn host_pixels(g: &Gpu, p: &Program, c: &Compiled, size: (u32, u32)) -> Vec<u8> {
    let bytes = loft_runner::bundle::encode(&portable_bundle(p, c, size));
    let mut r = runner(g);
    let id = r.load_bundle_bytes(&bytes).unwrap();
    let target = target(g, size);
    let mut encoder = g.device.create_command_encoder(&Default::default());
    r.encode_into(
        id,
        &mut encoder,
        &target.create_view(&Default::default()),
        None,
        0.0,
        0.0,
    )
    .unwrap();
    g.queue.submit([encoder.finish()]);
    loft_runner::read_texture(&g.device, &g.queue, &target, 0).unwrap()
}

#[test]
fn material_rows_share_code_and_the_cpu_query() {
    let source = scene("variety.loft");
    let (small, c) = compile(&source, &[("materials", 10.0)]);
    for n in [100.0, 1000.0] {
        let (large, _) = compile(&source, &[("materials", n)]);
        assert_eq!(large.materials[0].row_count(), n as u32);
        assert_eq!(large.instances.len(), 1000);
        assert_eq!(small.instances.len(), large.instances.len());
        for (a, b) in small.modules.iter().zip(&large.modules) {
            assert_eq!(a.wgsl, b.wgsl, "{}", a.name);
        }
    }
    let (two, _) = compile(&source, &[("materials", 200.0), ("families", 2.0)]);
    assert_eq!(two.kinds.len(), 2);
    let (all, _) = compile(&source, &[("families", 6.0)]);
    assert_eq!(all.kinds.len(), 6);

    let g = native::gpu().unwrap();
    let loaded = Loaded::new(&g, &small, (128, 80), None).unwrap();
    let pose = c.pose(0, 0).t;
    let point = [pose[0] + 0.017, pose[1], pose[2] + 0.06];
    let cpu = c.cpu_query(0, 0, point, [0.0, 0.0, 1.0]).unwrap();
    let gpu = loaded
        .twin(
            &g,
            0,
            &[point[0], point[1], point[2], 0.0, 0.0, 0.0, 1.0, 0.0],
        )
        .unwrap();
    for (cpu, gpu) in cpu.material.into_iter().zip(&gpu[4..7]) {
        assert!((cpu - gpu).abs() < 0.0001, "CPU {cpu}, GPU {gpu}");
    }
}

#[test]
fn material_upsert_changes_only_selected_hit_and_keeps_pipelines() {
    let source = scene("variety.loft");
    let (p, c) = compile(&source, &[("materials", 1000.0)]);
    let table = &p.materials[0];
    assert_eq!(table.stride_words, 16);
    let g = native::gpu().unwrap();
    let size = (640, 400);
    let loaded = Loaded::new(&g, &p, size, None).unwrap();
    loaded.frame(&g, 0.0);
    let before = loaded.pixels(&g).unwrap();
    let (vis, depth) = loaded.picture_planes(&g).unwrap();
    assert_eq!(host_pixels(&g, &p, &c, size), before);
    let count = loaded.pipeline_count().unwrap();

    let mut row = table.rows[..16].to_vec();
    row[..3].copy_from_slice(&[0.0, 12.0, 0.0]);
    let bytes = row
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    assert!(loaded.upsert_materials(&table.name, 1000, &bytes).is_err());
    assert!(
        loaded
            .upsert_materials(&table.name, 0, &bytes[..60])
            .is_err()
    );
    loaded.upsert_materials(&table.name, 0, &bytes).unwrap();
    loaded.frame(&g, 0.0);
    let after = loaded.pixels(&g).unwrap();
    assert_eq!(loaded.picture_planes(&g).unwrap(), (vis.clone(), depth));
    assert_eq!(loaded.pipeline_count().unwrap(), count);
    let mut changed = 0;
    for (i, (a, b)) in before
        .chunks_exact(4)
        .zip(after.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            assert_eq!(
                u32::from_le_bytes(vis[i * 8..i * 8 + 4].try_into().unwrap()),
                0
            );
            changed += 1;
        }
    }
    assert!(changed > 0);
    println!("material row 0 changed {changed} pixels, pipeline creations +0");
}

#[test]
fn atlas_pages_and_two_family_path_survive_bundle_round_trip() {
    let g = native::gpu().unwrap();
    let size = (128, 80);
    let atlas = scene("atlas.loft");
    let mut previous = None;
    for shuffle in [0.0, 1.0] {
        let (p, c) = compile(&atlas, &[("shuffle", shuffle)]);
        let loaded = Loaded::new(&g, &p, size, None).unwrap();
        loaded.frame(&g, 0.0);
        let pixels = loaded.pixels(&g).unwrap();
        assert_eq!(host_pixels(&g, &p, &c, size), pixels);
        if let Some(prev) = &previous {
            assert_ne!(prev, &pixels);
        }
        previous = Some(pixels);
    }

    let path = scene("bounces.loft");
    let center = ((size.1 / 2 * size.0 + size.0 / 2) * 4) as usize;
    let center_red = |bounces| {
        let (p, c) = compile(&path, &[("bounces", bounces)]);
        let loaded = Loaded::new(&g, &p, size, None).unwrap();
        loaded.frame_samples(&g, 0.0, 1);
        let pixels = loaded.pixels(&g).unwrap();
        assert_eq!(host_pixels(&g, &p, &c, size), pixels);
        pixels[center]
    };
    assert!(center_red(2.0) > center_red(0.0));

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bad = include_str!("../../../scenes/showcase/cap1c/bounces.loft")
        .replace("screen_pass(reference_sample,", "screen_pass(bad,");
    let bad =
        format!("{bad}\nfn bad(origin: v3, direction: v3, seed: u, bounces: u) -> f {{ 0.0 }}\n");
    let bad = Graph::text(&root, Path::new("bad-material-path.loft"), &bad);
    let Err(error) = loft_passes::compile(&bad, &[]) else {
        panic!("bad signature accepted")
    };
    assert!(matches!(error.code, Code::E003));
    assert!(
        error
            .render(&bad)
            .contains("invalid path function signature")
    );
}

#[test]
fn queued_frames_keep_their_graph_and_visibility_after_bundle_replacement() {
    let original = scene("variety.loft");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let edited = include_str!("../../../scenes/showcase/cap1c/variety.loft").replace(
        "standard_pbr(m.base, m.rough, m.metal, coords(p), m.grain, m.seed)",
        "emissive(rgb(0.1, 0.8, 0.1), 2.0)",
    );
    let edited = Graph::text(&root, Path::new("edited-material.loft"), &edited);
    let settings = [("materials", 10.0), ("families", 2.0)];
    let size = (256, 160);
    let (old, old_cpu) = compile(&original, &settings);
    let (new, new_cpu) = compile(&edited, &settings);
    let old = portable_bundle(&old, &old_cpu, size);
    let new = portable_bundle(&new, &new_cpu, size);
    let vis = |b: &loft_ir::Bundle| {
        b.resources
            .textures
            .iter()
            .find(|texture| {
                texture.format == loft_ir::ImageFormat::Rg32Uint && texture.extent[0] == size.0
            })
            .expect("current visibility plane")
            .table
    };

    let g = native::gpu().unwrap();
    let mut r = runner(&g);
    let id = r
        .load_bundle_bytes(&loft_runner::bundle::encode(&old))
        .unwrap();
    let [a, b] = [target(&g, size), target(&g, size)];
    let old_vis = r.export_texture(id, vis(&old)).unwrap();
    let mut previous = g.device.create_command_encoder(&Default::default());
    let before = r
        .encode_into(
            id,
            &mut previous,
            &a.create_view(&Default::default()),
            None,
            0.0,
            0.0,
        )
        .unwrap();

    let next = loft_runner::bundle::decode(&loft_runner::bundle::encode(&new)).unwrap();
    r.hot_swap_bundle(id, &next).unwrap();
    let new_vis = r.export_texture(id, vis(&new)).unwrap();
    let mut edited = g.device.create_command_encoder(&Default::default());
    let after = r
        .encode_into(
            id,
            &mut edited,
            &b.create_view(&Default::default()),
            None,
            0.0,
            0.0,
        )
        .unwrap();
    assert_eq!(before.instance, after.instance);
    assert!(before.resource_epoch < after.resource_epoch);
    g.queue.submit([previous.finish(), edited.finish()]);
    let pixels = |tex| loft_runner::read_texture(&g.device, &g.queue, tex, 0).unwrap();
    assert_eq!(pixels(&old_vis), pixels(&new_vis));
    assert_ne!(pixels(&a), pixels(&b));
}
