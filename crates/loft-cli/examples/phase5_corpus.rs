//! Score the authored corpus on the local adapter. The bead has a pinned geometry/depth oracle.
use loft_cli::native::{self, Loaded};
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
        src.add(p.display().to_string(), fs::read_to_string(p)?);
    }
    Ok(src)
}

fn main() -> Result<(), Box<dyn Error>> {
    let gpu = native::gpu()?;
    for name in std::env::args().skip(1) {
        let path = Path::new(&name);
        let src = sources(path)?;
        let (p, _, _) = loft_passes::compile(&src, &[]).map_err(|e| e.render(&src))?;
        let size = (512, 512);
        let control = gpu.triangle_control_ms(size)?;
        let loaded = Loaded::new(&gpu, &p, size, None)?;
        let mut ms = Vec::new();
        for frame in 0..16 {
            loaded.frame(&gpu, frame as f32 / 16.);
            ms.push(loaded.gpu_ms(&gpu)?.ok_or("missing GPU timestamps")?.0);
        }
        ms.sort_by(f64::total_cmp);
        println!(
            "{}",
            serde_json::json!({"scene":name,"p95_ms":ms[15],"control_p50_ms":control,"contended":control>0.35})
        );
        if path == Path::new("corpus/l5c/bead.loft") {
            let out = Path::new("out/meta/i5-fix/r0g");
            fs::create_dir_all(out)?;
            for (view, selector) in [("whole", 0.), ("through", 1.)] {
                let (mut p, _, _) = loft_passes::compile(&src, &[("view".into(), selector)])
                    .map_err(|e| e.render(&src))?;
                // Keep the submitted coverage, camera and hit distance. Replace only the display pass.
                // Raw label bytes go to an sRGB attachment, so undo its transfer function first.
                let mut code = String::from("@group(1) @binding(1) var vis_tex:texture_2d<u32>;\n");
                code.push_str("@vertex fn vs_present(@builtin(vertex_index) i:u32)->@builtin(position) vec4f {
                    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3));return vec4f(p[i],0,1); }
                    @fragment fn fs_present(@builtin(position) p:vec4f)->@location(0) vec4f {
                    let v=textureLoad(vis_tex,vec2i(p.xy),0);var label=0u;var distance=1e30;");
                let hash = |name: &str| {
                    name.bytes().fold(2166136261u32, |h, b| {
                        (h ^ u32::from(b)).wrapping_mul(16777619)
                    })
                };
                code += &format!(
                    "label={}u; if v.x != 0xffffffffu {{ distance=bitcast<f32>(v.y); switch v.x >> 24u {{",
                    hash("sky")
                );
                for (i, k) in p.kinds.iter().enumerate() {
                    let label = match k.name.as_str() {
                        "Bead" => "bead",
                        "Ground" => "ground",
                        other => return Err(format!("unknown bead label {other}").into()),
                    };
                    code += &format!("case {i}u: {{ label={}u; }}", hash(label));
                }
                code+="default: { discard; } } } let c=vec3f((vec3u(label>>16u,label>>8u,label)&vec3u(255u)))/255.;
                    let linear=select(pow((c+0.055)/1.055,vec3f(2.4)), c/12.92,c<=vec3f(0.04045));
                    return vec4f(linear,min(log2(1.+distance)/10.,1.)); }";
                p.modules[p.present].naga = naga::front::wgsl::parse_str(&code)?;
                for (w, h) in [(512, 512), (1280, 800)] {
                    if w == 1280 && selector == 1. {
                        continue;
                    }
                    let loaded = Loaded::new(&gpu, &p, (w, h), None)?;
                    for t in [0., 0.5, 1.37, 2.9] {
                        if w == 1280 && t != 1.37 {
                            continue;
                        }
                        loaded.frame(&gpu, t);
                        let pixels = loaded.pixels(&gpu)?;
                        let file = fs::File::create(out.join(format!("{view}@{t}@{w}x{h}.png")))?;
                        let mut enc = png::Encoder::new(file, w, h);
                        enc.set_color(png::ColorType::Rgba);
                        enc.set_depth(png::BitDepth::Eight);
                        enc.write_header()?.write_image_data(&pixels)?;
                    }
                }
            }
        }
    }
    Ok(())
}
