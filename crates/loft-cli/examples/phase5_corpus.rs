//! Time the authored corpus or capture an authored RGBA8 observation.
use loft_cli::native::{self, Gpu, Loaded};
use loft_ir::Sources;
use std::{error::Error, fs, path::Path};

fn sources(scene: &Path) -> Result<Sources, Box<dyn Error>> {
    let mut libs = fs::read_dir("lib")?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    libs.retain(|p| p.extension().is_some_and(|e| e == "loft"));
    libs.sort();
    let mut src = Sources::default();
    for p in libs.iter().map(AsRef::as_ref).chain([scene]) {
        add_source(&mut src, p)?;
    }
    Ok(src)
}

fn add_source(sources: &mut Sources, file: &Path) -> Result<(), std::io::Error> {
    sources.add(file.display().to_string(), fs::read_to_string(file)?);
    Ok(())
}

fn time_clip(gpu: &Gpu, name: &str) -> Result<(), Box<dyn Error>> {
    let src = sources(Path::new(name))?;
    let (p, _, _) = loft_passes::compile(&src, &[]).map_err(|e| e.render(&src))?;
    let size = (512, 512);
    let control = gpu.triangle_control_ms(size)?;
    let loaded = Loaded::new(gpu, &p, size, None)?;
    let mut ms = Vec::new();
    for frame in 0..16 {
        loaded.frame(gpu, frame as f32 / 16.);
        ms.push(loaded.gpu_ms(gpu)?.ok_or("missing GPU timestamps")?.0);
    }
    ms.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({"scene":name,"p95_ms":ms[15],"control_p50_ms":control,"contended":control>0.35})
    );
    Ok(())
}

fn write_rgba(gpu: &Gpu, tex: &wgpu::Texture, out: &Path) -> Result<(), Box<dyn Error>> {
    let (w, h) = (tex.width(), tex.height());
    let stride = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("observation readback"),
        size: u64::from(stride) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        tex.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: Some(h) },
        },
        tex.size(),
    );
    gpu.queue.submit([enc.finish()]);
    let (send, recv) = std::sync::mpsc::sync_channel(1);
    buf.map_async(wgpu::MapMode::Read, .., move |result| { let _ = send.send(result); });
    gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
    recv.recv()??;
    let pixels = buf.get_mapped_range(..)?;
    let mut encoder = png::Encoder::new(fs::File::create(out)?, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut png = encoder.write_header()?;
    let mut writer = png.stream_writer()?;
    use std::io::Write;
    for row in pixels.chunks_exact(stride as usize) {
        writer.write_all(&row[..(w * 4) as usize])?;
    }
    writer.finish()?;
    Ok(())
}

fn observe(gpu: &Gpu, args: &[String]) -> Result<(), Box<dyn Error>> {
    let [scene, size, time, output, set, recipes @ ..] = args else {
        return Err("usage: phase5_corpus --observe SCENE WIDTHxHEIGHT SECONDS PNG NAME=VALUE RECIPE...".into());
    };
    let (w, h) = size.split_once('x').ok_or("expected WIDTHxHEIGHT")?;
    let (w, h): (u32, u32) = (w.parse()?, h.parse()?);
    let (name, value) = set.split_once('=').ok_or("expected NAME=VALUE")?;
    let seconds: f32 = time.parse()?;
    if w == 0 || h == 0 || !seconds.is_finite() || seconds < 0.0 {
        return Err("observation requires a positive extent and a finite nonnegative time".into());
    }
    let mut src = sources(Path::new(scene))?;
    for file in recipes {
        add_source(&mut src, Path::new(file))?;
    }
    src.add(
        "<observation>",
        format!("bake_tex Observation {w}x{h} rgba8unorm on load mips(false) (uv: v2) {{\n  observe(uv, v2({w}.0, {h}.0), {seconds:?})\n}}"),
    );
    let (program, _, _) = loft_passes::compile(&src, &[(name.into(), value.parse()?)])
        .map_err(|e| e.render(&src))?;
    let loaded = Loaded::new(gpu, &program, (w, h), None)?;
    let image = loaded.baked_texture("Observation").ok_or("missing authored RGBA observation")?;
    write_rgba(gpu, &image, Path::new(output))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let gpu = native::gpu()?;
    match args.as_slice() {
        [mode, rest @ ..] if mode == "--observe" => observe(&gpu, rest),
        scenes => scenes.iter().try_for_each(|scene| time_clip(&gpu, scene)),
    }
}
