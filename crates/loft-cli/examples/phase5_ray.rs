//! Diagnostic for device negotiation and transformed triangle BLAS input.
use loft_cli::native::{self, Loaded, ray_query};
use loft_ir::Sources;
use wgpu::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut files = std::fs::read_dir("lib")?.map(|e| e.map(|e| e.path())).collect::<Result<Vec<_>, _>>()?;
    files.sort();
    let mut sources = Sources::default();
    for path in files.into_iter().filter(|p| p.extension().is_some_and(|e| e == "loft"))
        .chain(["crates/loft-passes/tests/phase5/anchor.loft".into()]) {
        sources.add(path.display().to_string(), std::fs::read_to_string(&path)?);
    }
    let (program, _, scene) = loft_passes::compile(&sources, &[]).map_err(|e| e.render(&sources))?;
    let triangles = scene.mesh_triangles().next().ok_or("missing imported triangles")?;
    let kind = program.kinds.iter().position(|k| k.name == "Mesh").ok_or("missing mesh kind")?;
    let row = 17;
    let pose = scene.pose(kind, row);
    let hit = [pose.t[0] + 0.25, pose.t[1] + 0.25, pose.t[2] + 1.];
    let miss = [pose.t[0] + 1.25, pose.t[1] + 1.25, pose.t[2] + 1.];
    let pack = |[x, y, z]: [f32; 3]| [x, y, z, f32::from_bits(row as u32), 0., 0., -1., 0.];
    let input = [pack(hit), pack(miss)].concat();
    let portable = native::gpu()?;
    let loaded = Loaded::new(&portable, &program, (1, 1), None)?;
    let distances = loaded.twin(&portable, kind, &input)?;
    let software = [distances[1], distances[17]];
    println!("{} portable per-row distances {software:?}", portable.adapter);

    let g = ray_query::request(ray_query::Requirements { geometries: 1, primitives: triangles.len().try_into()?, instances: 1 })?;
    let d = &g.device;
    let scope = d.push_error_scope(ErrorFilter::Validation);
    let mut enc = d.create_command_encoder(&Default::default());
    let mut transform = [0.; 12];
    for i in 0..3 {
        transform[i * 4..i * 4 + 3].copy_from_slice(&pose.r[i]);
        transform[i * 4 + 3] = pose.t[i];
    }
    let tree = ray_query::build(d, &mut enc, triangles, &[ray_query::Instance { transform, row: row as u32 }])?;
    let output = d.create_buffer(&BufferDescriptor {
        label: None, size: 24, usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let source = format!("{}\n{}", ray_query::WGSL, r#"
@group(0) @binding(0) var tree: acceleration_structure;
@group(0) @binding(1) var<storage, read_write> result: array<MeshRayHit>;
@compute @workgroup_size(1) fn main() {
    result[0] = mesh_ray_query(tree, vec3f(HIT), vec3f(0.0, 0.0, -1.0), 0.0, 10.0, false);
    result[1] = mesh_ray_query(tree, vec3f(MISS), vec3f(0.0, 0.0, -1.0), 0.0, 10.0, true);
}
"#.replace("HIT", &format!("{:?}, {:?}, {:?}", hit[0], hit[1], hit[2]))
   .replace("MISS", &format!("{:?}, {:?}, {:?}", miss[0], miss[1], miss[2])));
    let module = d.create_shader_module(ShaderModuleDescriptor {
        label: None, source: ShaderSource::Wgsl(source.into()),
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
        label: None, size: 24, usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    enc.copy_buffer_to_buffer(&output, 0, &read, 0, Some(24));
    g.queue.submit([enc.finish()]);
    d.poll(PollType::wait_indefinitely())?;
    if let Some(err) = pollster::block_on(scope.pop()) {
        return Err(err.into());
    }
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    read.map_async(MapMode::Read, .., move |result| { let _ = send.send(result); });
    d.poll(PollType::wait_indefinitely())?;
    receive.recv()??;
    let data = read.get_mapped_range(..)?;
    let actual = [f32::from_le_bytes(data[..4].try_into()?), f32::from_le_bytes(data[12..16].try_into()?)];
    let id = u32::from_le_bytes(data[4..8].try_into()?);
    let primitive = u32::from_le_bytes(data[8..12].try_into()?);
    println!("{} native BLAS distances {actual:?}, hit row {id}, triangle {primitive}", g.adapter);
    if (actual[0] - 1.0).abs() > 1e-5 || (actual[0] - software[0]).abs() > 1e-5
        || actual[1] != -1. || software[1] != -1. || id != row as u32 {
        return Err("transformed triangle diagnostic disagrees with the portable query".into());
    }
    Ok(())
}
