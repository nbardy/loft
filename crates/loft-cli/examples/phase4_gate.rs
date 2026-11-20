//! Integration measurements on the native device. Run under tools/gpu_lock.sh.
use loft_cli::native::{self, Gpu, Loaded};
use loft_ir::Sources;
use loft_passes::{Compiled, emit::Aov, program::{Draw, Program}};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn prelude() -> Sources {
    let mut paths: Vec<_> = fs::read_dir(root().join("lib"))
        .unwrap()
        .map(|x| x.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "loft"))
        .collect();
    paths.sort();
    let mut src = Sources::default();
    for p in paths {
        let name = p.strip_prefix(root()).expect("library path under the workspace");
        src.add(name.display().to_string(), fs::read_to_string(p).unwrap());
    }
    src
}

fn compile(source: &str) -> Result<(Program, Compiled), String> {
    let mut src = prelude();
    src.add("observation.loft", source.to_owned());
    loft_passes::compile(&src, &[])
        .map(|(p, _, c)| (p, c))
        .map_err(|e| e.render(&src))
}

fn scene(file: &str) -> Result<(Program, Compiled), String> {
    compile(&fs::read_to_string(root().join(file)).map_err(|e| e.to_string())?)
}

fn bake(g: &Gpu) -> Result<Value, String> {
    let (p, c) = scene("scenes/bake_load_fixture.loft")?;
    let l = Loaded::new(g, &p, (64, 64), None)?;
    let mut counts = Vec::new();
    for edit in [
        None,
        None,
        Some(("unrelated", 0.7)),
        Some(("density", 4.0)),
        Some(("density", 4.0)),
    ] {
        if let Some((name, value)) = edit {
            let index = c
                .prog()
                .param_word(name)
                .ok_or("missing watched parameter")?;
            l.set_param_word(index, value)?;
        }
        counts.push(l.frame(g, 0.));
        l.pixels(g)?;
    }
    Ok(
        json!({"load_ms":l.stats.load_bake_gpu_ms,"load_dispatches":l.stats.load_bake_commands,
        "load_pass_ms":l.stats.load_bake_pass_ms,"load_ticks":l.stats.load_bake_ticks,
        "commands_first_clean_unrelated_changed_repeated":counts,"expected_dirty_dispatches":5}),
    )
}

fn shrine(g: &Gpu) -> Result<Value, String> {
    let (p, _) = scene("engines/coral_shrine.loft")?;
    let size = (1280, 800);
    let control = g.triangle_control_ms(size)?;
    let l = Loaded::new(g, &p, size, None)?;
    let samples = l.profile_frames(g, std::iter::repeat_n(0., 130), true)?;
    let measured = &samples[30..];
    let mut times = measured.iter().map(|sample| sample.gpu_ms).collect::<Vec<_>>();
    times.sort_by(f64::total_cmp);
    Ok(
        json!({"control_p50_ms":control,"p50_ms":times[49],"p95_ms":times[94],
        "frames":100,"warmup":30,"camera_radius":p.camera.radius,"camera_eye":p.camera.eye(0.0),"instances":p.instances.len(),"schedule":"two-world woodblock",
        "passes":p.passes.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>(),
        "gpu_pass_ms":measured.iter().map(|s| &s.pass_ms).collect::<Vec<_>>(),"frame_stamps":measured,
        "modules":loft_passes::telemetry::collect(&p)}),
    )
}

fn hermit_source() -> Result<Sources, String> {
    let mut src = prelude();
    let file = "engines/hermit.loft";
    src.add(file, fs::read_to_string(root().join(file)).map_err(|e| e.to_string())?);
    Ok(src)
}

fn hermit_load(g: &Gpu, count: u32, nonce: u32) -> Result<Value, String> {
    let cache = tempfile::tempdir().map_err(|e| e.to_string())?;
    let overrides = [("count".into(), f64::from(count))];
    let mut runs = Vec::new();
    // Match the Load gate's derivation cache and stop the clock before drawing a frame.
    for temperature in ["cold", "warm"] {
        let start = std::time::Instant::now();
        let src = hermit_source()?;
        let (program, report, _) = loft_passes::compile_cached(&src, &overrides, cache.path())
            .map_err(|e| e.render(&src))?;
        let front = start.elapsed().as_secs_f64();
        let loaded = Loaded::new(g, &program, (1280, 800), Some(nonce))?;
        let total = start.elapsed().as_secs_f64();
        let stats = &loaded.stats;
        runs.push(json!({
            "temperature": temperature, "seconds": total, "front_seconds": front,
            "pipeline_ms": stats.pipelines_ms, "resources_ms": stats.resources_ms,
            "modules": stats.modules, "creates": stats.pipeline_ms,
            "load_bake_dispatches": stats.load_bake_commands,
            "derivation_hits": report.kinds.iter().map(|k| k.cache_hits).sum::<usize>(),
        }));
    }
    Ok(json!({"adapter": g.adapter, "count": count, "nonce": nonce, "loads": runs}))
}

fn hermit_grid(g: &Gpu, dir: &Path, archive: Option<&Path>) -> Result<Value, String> {
    let src = hermit_source()?;
    let size = (1280, 800);
    let archive = archive.map(|path| {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        loft_passes::sexp::read(&text)
    }).transpose()?;
    let mut reports = Vec::new();
    for count in [1, 20] {
        for (label, aov) in [("beauty", Aov::Beauty), ("depth", Aov::Depth), ("id", Aov::Id)] {
            let (mut program, _, _) = loft_passes::compile_aov(&src, &[("count".into(), f64::from(count))], aov)
                .map_err(|e| e.render(&src))?;
            let surface = program.kinds.iter().find(|k| matches!(k.draw, Draw::Surface { .. }))
                .ok_or("missing Hermit surface")?.module;
            let entries = &program.modules[surface].naga.entry_points;
            if !entries.iter().any(|e| e.name == "vs_grid") || entries.iter().any(|e| e.name == "bake_surface") {
                return Err("expected a bake-free indexed Hermit surface".into());
            }
            let indexed = Loaded::with_aov(g, &program, size, None, aov)?;
            // Remove only the index-only entry to exercise the original procedural corners.
            program.modules[surface].naga.entry_points.retain(|e| e.name != "vs_grid");
            let corners = Loaded::with_aov(g, &program, size, None, aov)?;
            let mut variants = vec![("indexed", indexed), ("corners", corners)];
            if let Some(archive) = archive.as_ref().filter(|_| count == 20 && aov == Aov::Beauty) {
                if !archive.modules.iter().any(|m| m.naga.entry_points.iter().any(|e| e.name == "bake_surface")) {
                    return Err("expected an archived count-20 program with the baked surface entry".into());
                }
                variants.push(("archive", Loaded::new(g, archive, size, None)?));
            }
            let mut frames = Vec::new();
            for (variant, loaded) in variants {
                let commands = loaded.frame(g, 0.0);
                let pixels = loaded.pixels(g)?;
                let file = fs::File::create(dir.join(format!("hermit-grid-{count}-{label}-{variant}.png")))
                    .map_err(|e| e.to_string())?;
                let mut png = png::Encoder::new(file, size.0, size.1);
                png.set_color(png::ColorType::Rgba);
                png.set_depth(png::BitDepth::Eight);
                png.write_header().and_then(|mut writer| writer.write_image_data(&pixels))
                    .map_err(|e| e.to_string())?;
                let stats = &loaded.stats;
                frames.push((pixels, json!({
                    "variant": variant, "commands": commands, "modules": stats.modules,
                    "creates": stats.pipeline_ms, "load_bake_dispatches": stats.load_bake_commands,
                })));
            }
            let indexed = &frames[0].0;
            let comparisons: Vec<_> = frames[1..].iter().map(|(pixels, stats)| json!({
                "variant": stats["variant"],
                "different_pixels": indexed.chunks_exact(4).zip(pixels.chunks_exact(4))
                    .filter(|(a, b)| a != b).count(),
            })).collect();
            reports.push(json!({
                "count": count, "mode": label, "time": 0, "comparisons": comparisons,
                "runs": frames.into_iter().map(|(_, stats)| stats).collect::<Vec<_>>(),
            }));
        }
    }
    Ok(json!({"adapter": g.adapter, "frames_compared": reports}))
}

fn observe(
    g: &Gpu,
    p: &mut Program,
    body: &str,
    samples: u32,
    cases: usize,
) -> Result<Value, String> {
    // Keep production queries, material evaluation, integrator and runtime bindings. Replace only the
    // fragment observation: encode one linear f32 into four UNORM bytes before exposure or tonemapping.
    let m = &mut p.modules[p.present];
    let (prefix, _) = m
        .wgsl
        .split_once("@fragment fn fs_present")
        .ok_or("missing present entry")?;
    let shader = format!("{prefix}\n@fragment fn fs_present(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {{
        let x = u32(pos.x); let ch = x % 3u; let idx = x / 3u;
        let seed = path_hash(u32(pos.y)*{samples}u + idx);
        let test = u32(pos.y);
        {body}
        let bits = bitcast<u32>(value);
        return vec4<f32>((vec4<u32>(bits) >> vec4<u32>(0,8,16,24)) & vec4<u32>(255)) / 255.0;
    }}");
    m.naga = naga::front::wgsl::parse_str(&shader).map_err(|e| e.emit_to_string(&shader))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&m.naga)
    .map_err(|e| e.emit_to_string(&shader))?;
    let l = Loaded::with_aov(g, p, (samples * 3, cases as u32), None, Aov::Normal)?;
    l.frame(g, 0.);
    let px = l.pixels(g)?;
    let mut mean = vec![[0.0; 3]; cases];
    for (row, buf) in px.chunks_exact(samples as usize * 12).enumerate() {
        for (i, bytes) in buf.chunks_exact(4).enumerate() {
            let f = f32::from_le_bytes(bytes.try_into().unwrap());
            if !f.is_finite() {
                return Err(format!("nonfinite sample at ({i},{row})"));
            }
            mean[row][i % 3] += f64::from(f) / f64::from(samples);
        }
    }
    Ok(json!({"linear_rgb":mean,"paths_per_case":samples}))
}

const WALL: &str = "
use engines::reference
space World
kind Wall @World () {
  trace(p) = box_sdf(p - [0,0,0]@World, [1,1,1])
  material(p,n) = lambert([0.5,0.5,0.5]) + emit_lobe([1,1,1])
}
build { emit Wall()
emit Sun(dir: dir([0,1,0]@World), color: [0,0,0])
emit Sky(zenith: [0,0,0], horizon: [0,0,0]) }
";
const SPHERE: &str = "
use engines::reference
space World
kind Ball @World () {
  visible(ro,rd) = ray_sphere(ro,rd,[0,0,0]@World,1)
  trace(p) = sphere(p - [0,0,0]@World,1)
  material(p,n) = ggx([1,1,1],1,1)
}
build { emit Ball()
emit Sun(dir: dir([0,1,0]@World), color: [0,0,0])
emit Sky(zenith: [0.5,0.5,0.5], horizon: [0.5,0.5,0.5]) }
";

fn quant(g: &Gpu) -> Result<Value, String> {
    use loft_ir::{DepthConvention, GpuFeatures, HostCaps, Span};
    use loft_passes::eval::{Eval, Val};
    let mut src = prelude();
    let demo = "
use std::quant::*
fn sums(a:u,b:u) -> v2 = [f32(dot4_i8(a,b)),f32(dot4_u8(a,b))]
fn bf(a:u) -> v2 = unpack_bf16x2(a)
fn lo(a:u) -> v4 = unpack_u4_lo(a)
fn hi(a:u) -> v4 = unpack_u4_hi(a)
fn retain_all(a:u,b:u) -> rgb = [sums(a,b).x,bf(a).y,lo(a).x + hi(a).y]
";
    src.add("quant.loft", demo.to_owned());
    src.add(
        "sphere.loft",
        SPHERE.replace(
            "ggx([1,1,1],1,1)",
            "lambert(retain_all(0x7f00ff80,0x7f80fd02))",
        ),
    );
    let probe = "
        let word = 0xfedcba98u;
        let s = f32(loft_quant_dot4_i8(0x7f00ff80u,0x7f80fd02u));
        let u = f32(loft_quant_dot4_u8(0x7f00ff80u,0x7f80fd02u));
        let bf = loft_quant_unpack_bf16x2(0xc0203f80u);
        let lo = loft_quant_unpack_u4_lo(word);
        let hi = loft_quant_unpack_u4_hi(word);
        let v = array(vec3(s,u,bf.x),vec3(bf.y,lo.x,lo.y),vec3(lo.z,lo.w,hi.x),hi.yzw);
        let value = v[test][ch];";
    let mut result = json!({});
    for (label, features) in [
        ("portable", GpuFeatures::empty()),
        ("packed", GpuFeatures::PACKED_INT8_DOT),
    ] {
        let caps = HostCaps {
            features,
            msaa_samples: 4,
            depth_convention: DepthConvention::StandardZeroToOne,
        };
        let (mut p, _, c) =
            loft_passes::quant::compile(&src, &[], caps).map_err(|d| d.render(&src))?;
        let shader = &p.modules[p.present].wgsl;
        let packed = features.contains(GpuFeatures::PACKED_INT8_DOT);
        if shader.contains("dot4I8Packed(a, b)") != packed
            || shader.contains("dot4U8Packed(a, b)") != packed
        {
            return Err(format!("wrong packed dot emission for {label}"));
        }
        let prog = c.prog();
        let mut twin = Eval::<f32>::new(&prog, &c.types, vec![], &[]);
        let mut expected = Vec::new();
        for (name, args) in [
            ("sums", vec![0x7f00ff80, 0x7f80fd02]),
            ("bf", vec![0xc0203f80]),
            ("lo", vec![0xfedcba98]),
            ("hi", vec![0xfedcba98]),
        ] {
            let vals = args.into_iter().map(Val::U).collect();
            let Val::V(v, _) = twin
                .call_fn(prog.fns[name], vals, Span::NONE)
                .map_err(|e| e.render(&src))?
            else {
                return Err(format!("expected a twin vector from {name}"));
            };
            expected.extend(v.into_iter().map(f64::from));
        }
        let device = observe(g, &mut p, probe, 1, 4)?;
        let actual: Vec<[f64; 3]> =
            serde_json::from_value(device["linear_rgb"].clone()).map_err(|e| e.to_string())?;
        let actual = actual.as_flattened();
        result[label] = json!({"gpu":actual,"cpu":expected,"match":actual == expected,
            "naga_validated":true,"packed_dot_emitted":packed});
    }
    Ok(result)
}

fn measure(g: &Gpu, gate: &str) -> Result<Value, String> {
    match gate {
        "quant" => quant(g),
        "bake" => bake(g),
        "shrine" => shrine(g),
        "spectral-slots" => {
            let (program, _) = scene("scenes/spectral_caustic_study.loft")?;
            let usage = loft_passes::telemetry::storage_entries(&program.modules[program.present].naga)?;
            let loaded = Loaded::new(g, &program, (128, 128), None)?;
            let commands = loaded.frame_samples(g, 0.0, 4);
            let bytes = loaded.pixels(g)?;
            Ok(json!({"device_limit":g.device.limits().max_storage_buffers_per_shader_stage,
                "entry_storage":usage, "rendered_rgba_bytes":bytes.len(), "samples":4, "commands":commands}))
        }
        "closed" => observe(g,&mut compile(WALL)?.0,
            "let depth = select(select(1u,2u,test==1u),16u,test==2u);
             let value = loft_user_reference_sample(vec3(0.),normalize(vec3(0.1,0.3,1.)),seed,depth)[ch];",1024,3),
        "white" => observe(g,&mut compile(SPHERE)?.0,
            "let o = vec3((select(16.5,29.5,test==1u)-16.)/16.,0.5/16.,3.);
             let value = loft_user_reference_sample(o,vec3(0.,0.,-1.),seed,1u)[ch];",1024,2),
        "lights" => {
            // Each point is 2 m away along the normal. Intensity 4*pi cancels inverse square and Lambert 1/pi.
            let scene = SPHERE.replace("ggx([1,1,1],1,1)","lambert([1,1,1])")
                .replace("emit Ball()", "emit Ball()\nbuild for i in 0..1000 { emit PointLamp(pos: [0,0,3]@World, color: rgb(12.5663706 * (0.05 + f32(i % 29)/28))) }");
            observe(g,&mut compile(&scene)?.0,"let h = world_trace(vec3(0.,0.,4.),vec3(0.,0.,-1.),10.);
                let value = world_direct(h,vec3(0.,0.,1.),seed,0u)[ch];",1024,32)
        }
        "authored-media" => {
            let source = r#"
space World Local
world Fog { things: [FieldMedium] }
world Vacuum { things: [] }
fn authored(w: World, r: Ray, rng: RngKey) -> Vol = inscatter_joint(w, r, 1m, 24, rng)
fn authored_visibility(w: World, a: p3@World, b: p3@World) -> rgb = visible(w, a, b)
fn sample_ray(seed: u) -> Ray =
  wgsl(seed: u) -> Ray {
    return loft_type_Ray(vec3(0.0), vec3(0.0,0.0,1.0), 0.0, 1.0, vec2(0.0));
  }
fn sample_key(seed: u) -> RngKey =
  wgsl(seed: u) -> RngKey {
    return array<u32,4>(seed,0u,0u,0u);
  }
fn measurement(o: v3, d: v3, seed: u, bounces: u) -> rgb {
  let ray = sample_ray(seed)
  let vol = authored(Fog, ray, sample_key(seed))
  vol.l + authored_visibility(Vacuum, ray.o, ray.o + ray.d) * vol.t
}
schedule Main {
  orbit(at: [0,0,0], radius: 3, height: 0.5, fov: 40deg)
  cull_indirect()
  render_bundle(Vis64) {}
  tile_classify(Vis64, tile: 8)
  shade_tiles(Vis64, light: sun_sky, sun: [0,1,0])
  screen_pass(measurement, bounces: 0)
  tonemap_aces(ColorHdr, sky: sky_color)
}
"#;
            // Call the checked Loft functions from the linear readback probe.
            // Red/blue row permutation must give the same L and T and leave the second world in vacuum.
            let rows = [("0.69314718", "[0.69314718,0,0]"), ("0.69314718", "[0,0,0.69314718]")];
            let mut results = Vec::new();
            for order in [rows, [rows[1], rows[0]]] {
                let mut text = format!("{source}\nbuild {{\n");
                for (extinction, emission) in order {
                    text += &format!("emit FieldMedium(lo: [-1,-1,0]@World, hi: [1,1,1]@World, sigma_t: rgb({extinction}), albedo: [0,0,0], emission: {emission})\n");
                }
                text += "}\n";
                let (mut p, _) = compile(&text)?;
                results.push(observe(g, &mut p, r"
                    let ray = loft_type_Ray(vec3(0.0), vec3(0.0,0.0,1.0), 0.0, 1.0, vec2(0.0));
                    let vol = loft_user_authored(1u, ray, array<u32,4>(seed,0u,0u,0u));
                    let vis = loft_user_authored_visibility(select(1u,2u,test==3u),ray.o,ray.o+ray.d);
                    let value = select(select(vol.t[ch],vol.l[ch],test==0u),vis[ch],test>=2u);
                ", 32, 4)?);
            }
            let target = [[0.375,0.0,0.375],[0.25;3],[0.25;3],[1.0;3]];
            for result in &results {
                let measured: Vec<[f64;3]> = serde_json::from_value(result["linear_rgb"].clone()).map_err(|e| e.to_string())?;
                if measured.len()!=target.len() || measured.iter().zip(target).any(|(got, expected)| (0..3).any(|i| (got[i]-expected[i]).abs()>1e-4)) {
                    return Err(format!("authored media differs from slab oracle: {result}"));
                }
            }
            Ok(json!({"orders":results,"case_order":["radiance","transmittance","visibility","vacuum_visibility"]}))
        }
        "media" => {
            let mut p = compile(SPHERE)?.0;
            let m = &mut p.modules[p.present];
            // Supply the two World coefficients in both orders to the shipped WGSL marcher.
            let (_, marcher) = loft_passes::bsdf::INSCATTER_WGSL.split_once("struct LoftCoefficients")
                .expect("joint marcher coefficients");
            m.wgsl = format!("struct LoftCoefficients{}\nfn loft_sample_medium(o:vec3<f32>,d:vec3<f32>,t:f32,k:u32,r:vec4<u32>)->LoftCoefficients {{
                let sigma=log(2.); let a=select(vec3(sigma,0.,0.),vec3(0.,0.,sigma),r.x==1u);
                let b=select(vec3(0.,0.,sigma),vec3(sigma,0.,0.),r.x==1u);
                return LoftCoefficients(vec3(sigma+sigma),a+b);
            }}\n{}",marcher,m.wgsl);
            observe(g,&mut p,"let m = loft_inscatter_joint(vec3(0.),vec3(0.,0.,1.),0.,1.,24u,vec4(test/2u));
                let value = select(m.l,m.t,test%2u==1u)[ch];",1,4)
        }
        _ => Err(format!("unknown observation {gate}")),
    }
}

fn main() -> Result<(), String> {
    let gate = std::env::args().nth(1).ok_or("supply an observation")?;
    let dir = std::env::args().nth(2).unwrap_or_else(|| "out/meta/i4".into());
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let gpu = native::gpu()?;
    let result = match gate.as_str() {
        "hermit-grid" => {
            let archive = std::env::args_os().nth(3);
            hermit_grid(&gpu, Path::new(&dir), archive.as_deref().map(Path::new))?
        }
        "hermit-load" => {
            let count = std::env::args().nth(3).and_then(|s| s.parse().ok())
                .ok_or("expected Hermit count")?;
            let nonce = std::env::args().nth(4).and_then(|s| s.parse().ok())
                .filter(|n| !matches!(*n, 0 | u32::MAX)).ok_or("expected cold-load nonce")?;
            hermit_load(&gpu, count, nonce)?
        }
        _ => measure(&gpu, &gate)?,
    };
    fs::write(
        Path::new(&dir).join(format!("{gate}.json")),
        result.to_string(),
    )
    .map_err(|e| e.to_string())
}
