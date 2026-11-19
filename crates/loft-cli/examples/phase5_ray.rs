use loft_cli::native::{self, ray_query};
use loft_ir::Sources;
use wgpu::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let g = native::request_gpu(true)?;
    let d = &g.device;
    let mut files = std::fs::read_dir("lib")?.map(|e| e.map(|e| e.path())).collect::<Result<Vec<_>, _>>()?;
    files.sort();
    let mut sources = Sources::default();
    for path in files.into_iter().filter(|p| p.extension().is_some_and(|e| e == "loft"))
        .chain(["crates/loft-passes/tests/phase5/anchor.loft".into()]) {
        sources.add(path.display().to_string(), std::fs::read_to_string(&path)?);
    }
    let (_, _, scene) = loft_passes::compile(&sources, &[]).map_err(|e| e.render(&sources))?;
    let triangles = scene.mesh_triangles().next().ok_or("missing imported triangles")?;
    let mut enc = d.create_command_encoder(&Default::default());
    let tree = ray_query::build(d, &mut enc, triangles)?;
    let output = d.create_buffer(&BufferDescriptor {
        label: None, size: 8, usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let module = d.create_shader_module(ShaderModuleDescriptor {
        label: None, source: ShaderSource::Wgsl(format!("{}\n{}", ray_query::WGSL, r#"
@group(0) @binding(0) var tree: acceleration_structure;
@group(0) @binding(1) var<storage, read_write> result: array<f32>;
@compute @workgroup_size(1) fn main() {
    result[0] = mesh_ray_query(tree, vec3f(0.25, 0.25, 1.0), vec3f(0.0, 0.0, -1.0), 10.0);
    result[1] = mesh_ray_query(tree, vec3f(2.0, 2.0, 1.0), vec3f(0.0, 0.0, -1.0), 10.0);
}
"#).into()),
    });
    let pipeline = d.create_compute_pipeline(&ComputePipelineDescriptor {
        label: None, layout: None, module: &module, entry_point: Some("main"),
        compilation_options: Default::default(), cache: None,
    });
    let bg = d.create_bind_group(&BindGroupDescriptor {
        label: None, layout: &pipeline.get_bind_group_layout(0),
        entries: &[BindGroupEntry { binding: 0, resource: tree.as_binding() },
            BindGroupEntry { binding: 1, resource: output.as_entire_binding() }],
    });
    {
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    let read = d.create_buffer(&BufferDescriptor {
        label: None, size: 8, usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    enc.copy_buffer_to_buffer(&output, 0, &read, 0, Some(8));
    g.queue.submit([enc.finish()]);
    read.map_async(MapMode::Read, .., |r| r.unwrap());
    d.poll(PollType::wait_indefinitely())?;
    let data = read.get_mapped_range(..)?;
    let actual = [f32::from_le_bytes(data[..4].try_into()?), f32::from_le_bytes(data[4..8].try_into()?)];
    let cpu = scene.cpu_query(0, 0, [0.25, 0.25, 1.], [0., 0., -1.]).map_err(|e| e.render(&sources))?;
    assert!((actual[0] - cpu.visible).abs() <= 1e-5);
    assert_eq!(actual[1], -1.);
    println!("{} hardware hit {:.6} m, CPU {:.6} m, miss {}", g.adapter, actual[0], cpu.visible, actual[1]);
    Ok(())
}
