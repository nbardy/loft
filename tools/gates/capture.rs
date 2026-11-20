// Build with pictures.py against the same lab artifacts as the measured Loft binary.
#[allow(dead_code, unused_imports)]
mod native;
use loft_passes::{program::Program, source::Graph};
use native::Loaded;
use serde_json::Value;
use std::{error::Error, fs, path::Path};

fn order_cull(program: &mut Program) -> Result<(), Box<dyn Error>> {
    // Equal D32 depths retain the first draw. Give compact cull output the same
    // stable row order as the existing control, preserving the arm's frustum test.
    let shader = &mut program.modules[program.cull];
    let compact = loft_passes::emit::cull_module(false);
    let indexed = loft_passes::emit::cull_module(true);
    let boundary = "  if outside != 0u { return; }";
    let (prefix, _) = compact
        .split_once(boundary)
        .ok_or("missing compact cull tail")?;
    let indexed_tail = indexed
        .strip_prefix(prefix)
        .ok_or("unexpected indexed cull shader")?;
    let compact_tail = &compact[prefix.len()..];
    match (
        shader.wgsl.matches(compact_tail).count(),
        shader.wgsl.matches(indexed_tail).count(),
    ) {
        (1, 0) => {
            shader.wgsl = shader.wgsl.replace(compact_tail, indexed_tail);
            shader.naga = naga::front::wgsl::parse_str(&shader.wgsl)?;
        }
        (0, 1) => {}
        _ => return Err("unrecognized cull ordering for deterministic pictures".into()),
    }
    Ok(())
}

fn capture(root: &Path, out: &Path) -> Result<(), Box<dyn Error>> {
    let manifest: Value = serde_json::from_slice(&fs::read(root.join("engines/gates.json"))?)?;
    let size: (u32, u32) = serde_json::from_value(manifest["size"].clone())?;
    let gpu = native::gpu()?;
    println!("adapter {}", gpu.adapter);
    fs::write(out.join("adapter.txt"), &gpu.adapter)?;
    for scene in manifest["scenes"].as_array().ok_or("missing scenes")? {
        let name = scene["name"].as_str().ok_or("missing scene name")?;
        let file = root.join(scene["file"].as_str().ok_or("missing scene file")?);
        let settings = serde_json::from_value::<std::collections::BTreeMap<String, f64>>(
            scene["set"].clone(),
        )?;
        let times = scene.get("picture_frames").unwrap_or(&manifest["frames"]);
        let times: Vec<f32> = serde_json::from_value(times.clone())?;
        let graph = Graph::file(&file, "")?;
        let (mut program, _, _) =
            loft_passes::compile(&graph, &settings.into_iter().collect::<Vec<_>>())
                .map_err(|error| error.render(&graph))?;
        order_cull(&mut program)?;
        let beauty = Loaded::new(&gpu, &program, size, None)?;
        for time in times {
            let stem = format!("{name}_t{time}");
            beauty.frame(&gpu, time);
            let mut png = png::Encoder::new(
                fs::File::create(out.join(format!("{stem}.png")))?,
                size.0,
                size.1,
            );
            png.set_color(png::ColorType::Rgba);
            png.set_depth(png::BitDepth::Eight);
            png.write_header()?
                .write_image_data(&beauty.pixels(&gpu)?)?;
            let (id, depth) = beauty.picture_planes(&gpu)?;
            fs::write(out.join(format!("{stem}.id.u32")), id)?;
            fs::write(out.join(format!("{stem}.depth.f32")), depth)?;
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let [root, out] = args.as_slice() else {
        return Err("usage: capture CHECKOUT OUTPUT_DIR".into());
    };
    capture(Path::new(root), Path::new(out))
}
