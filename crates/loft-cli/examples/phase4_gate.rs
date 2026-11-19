//! Integration measurements on the native device. Run under tools/gpu_lock.sh.
use loft_cli::native::{self, Gpu, Loaded};
use loft_ir::Sources;
use loft_passes::{Compiled, emit::Aov, program::Program};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn prelude() -> Sources {
    let mut paths: Vec<_> = fs::read_dir("lib")
        .unwrap()
        .map(|x| x.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "loft"))
        .collect();
    paths.sort();
    let mut src = Sources::default();
    for p in paths {
        src.add(p.display().to_string(), fs::read_to_string(p).unwrap());
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
    compile(&fs::read_to_string(file).map_err(|e| e.to_string())?)
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
        "commands_first_clean_unrelated_changed_repeated":counts,"expected_dirty_dispatches":5}),
    )
}

fn shrine(g: &Gpu) -> Result<Value, String> {
    let (p, _) = scene("engines/coral_shrine.loft")?;
    let size = (1280, 800);
    let control = g.triangle_control_ms(size)?;
    let l = Loaded::new(g, &p, size, None)?;
    let mut times = Vec::new();
    for i in 0..130 {
        l.frame(g, 0.);
        let ms = l.gpu_ms(g)?.ok_or("missing GPU timestamps")?.0;
        if i >= 30 {
            times.push(ms);
        }
    }
    times.sort_by(f64::total_cmp);
    Ok(
        json!({"control_p50_ms":control,"p50_ms":times[49],"p95_ms":times[94],
        "frames":100,"warmup":30,"camera_radius":p.camera.radius,"instances":p.instances.len(),"schedule":"authored raster"}),
    )
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

fn measure(g: &Gpu, gate: &str) -> Result<Value, String> {
    match gate {
        "bake" => bake(g),
        "shrine" => shrine(g),
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
        "media" => {
            let mut p = compile(SPHERE)?.0;
            let m = &mut p.modules[p.present];
            // Supply the two World coefficients in both orders to the shipped WGSL marcher.
            m.wgsl = format!("{}\nfn loft_sample_medium(o:vec3<f32>,d:vec3<f32>,t:f32,k:u32,r:vec4<u32>)->LoftCoefficients {{
                let sigma=log(2.); let a=select(vec3(sigma,0.,0.),vec3(0.,0.,sigma),r.x==1u);
                let b=select(vec3(0.,0.,sigma),vec3(sigma,0.,0.),r.x==1u);
                return LoftCoefficients(vec3(sigma+sigma),a+b);
            }}\n{}",loft_passes::bsdf::INSCATTER_WGSL,m.wgsl);
            observe(g,&mut p,"let m = loft_inscatter_joint(vec3(0.),vec3(0.,0.,1.),0.,1.,24u,vec4(test/2u));
                let value = select(m.l,m.t,test%2u==1u)[ch];",1,4)
        }
        _ => Err(format!("unknown observation {gate}")),
    }
}

fn main() -> Result<(), String> {
    let gate = std::env::args().nth(1).ok_or("supply an observation")?;
    let result = measure(&native::gpu()?, &gate)?;
    fs::write(
        Path::new("out/meta/i4").join(format!("{gate}.json")),
        result.to_string(),
    )
    .map_err(|e| e.to_string())
}
