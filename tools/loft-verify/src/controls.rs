use loft_cli::{budget::{self, Failure}, flag, sources, write_json};
use loft_cli::native::{self, FrameTiming, Gpu, Loaded, QUIET_CONTROL_P50_MS};
use loft_passes::{
    emit::Aov,
    program::{Pass, Program, ShaderModule},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{path::Path, time::Instant};

const SIZE: (u32, u32) = (1280, 800);
const WARMUP: usize = 30;
const LIMIT: f64 = 1.05;
const BOOTSTRAPS: usize = 4096;
const BLOCK: usize = 10;
const HERMIT: &str = include_str!("../../../tests/controls/hermit_shade.wgsl");
const SHRINE: &str = include_str!("../../../tests/controls/shrine_vis.wgsl");
const READBACK: &str = include_str!("../../../tests/controls/readback.wgsl");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fixture {
    Hermit,
    Shrine,
}

impl Fixture {
    fn name(self) -> &'static str {
        match self {
            Self::Hermit => "hermit_shade",
            Self::Shrine => "shrine_vis",
        }
    }

    fn scene(self) -> (&'static str, &'static [(&'static str, f64)]) {
        match self {
            Self::Hermit => ("engines/hermit.loft", &[("count", 20.0)]),
            Self::Shrine => (
                "engines/coral_shrine.loft",
                &[("shrine_distance", 6.4), ("shrine_solid", 0.0)],
            ),
        }
    }

    fn source(self) -> &'static str {
        match self {
            Self::Hermit => HERMIT,
            Self::Shrine => SHRINE,
        }
    }

    fn frames(self) -> usize {
        match self {
            Self::Hermit => 300,
            Self::Shrine => 100,
        }
    }

    fn time(self, i: usize) -> f32 {
        match self {
            Self::Hermit => i as f32 * (1.0 / 60.0),
            Self::Shrine => 0.0,
        }
    }

    fn subject(self, pass: &Pass) -> bool {
        matches!(
            (self, pass),
            (Self::Hermit, Pass::Shade(_)) | (Self::Shrine, Pass::Vis(_))
        )
    }

    fn kinds(self) -> &'static [(&'static str, u32, &'static str, &'static str)] {
        // name, row stride in f32 words, control vertex and selected fragment/compute entry
        match self {
            Self::Hermit => &[
                ("Ground", 8, "", "ground_shade"),
                ("HermitShell", 22, "", "shell_shade"),
                ("CrabFace", 26, "", "face_shade"),
                ("CrusherClaw", 25, "", "claw_shade"),
                ("CrabLeg", 34, "", "leg_shade"),
            ],
            Self::Shrine => &[
                ("ShrineFloor", 7, "floor_proxy", "floor_hit"),
                ("FoldedLantern", 20, "lantern_proxy", "lantern_hit"),
                ("CoralBranch", 21, "branch_proxy", "branch_hit"),
            ],
        }
    }
}

fn parse(source: &str) -> Result<naga::Module, Failure> {
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|e| Failure::Compile(e.emit_to_string(source)))?;
    validate(&module, source)?;
    Ok(module)
}

fn validate(module: &naga::Module, source: &str) -> Result<naga::valid::ModuleInfo, Failure> {
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(module)
    .map_err(|e| Failure::Compile(e.emit_to_string(source)))
}

fn rename(module: &mut naga::Module, old: &str, new: &str) -> Result<(), Failure> {
    let entry = module
        .entry_points
        .iter_mut()
        .find(|e| e.name == old)
        .ok_or_else(|| Failure::Compile(format!("missing shader entry {old}")))?;
    entry.name = new.to_owned();
    Ok(())
}

fn joined(
    fixture: Fixture,
    baseline: &ShaderModule,
    vertex: &str,
    entry: &str,
    depth: bool,
) -> Result<ShaderModule, Failure> {
    let hand = fixture.source();
    // Link only at the entry boundary. The hand shader has no dependencies on compiler helpers.
    let wgsl = format!("{}\n{hand}", baseline.wgsl);
    let wgsl = if depth { depth_record(&wgsl)? } else { wgsl };
    let mut naga = parse(&wgsl)?;
    match fixture {
        Fixture::Hermit => {
            rename(&mut naga, "shade", "baseline_shade")?;
            rename(&mut naga, entry, "shade")?;
        }
        Fixture::Shrine => {
            rename(&mut naga, "vs", "baseline_vs")?;
            rename(&mut naga, "fs", "baseline_fs")?;
            rename(&mut naga, vertex, "vs")?;
            rename(&mut naga, entry, "fs")?;
        }
    }
    // Keep only the names the runner can select. Bindings in each entry stay self-contained.
    naga.entry_points.retain(|e| {
        baseline
            .naga
            .entry_points
            .iter()
            .any(|old| old.name == e.name)
    });
    validate(&naga, &wgsl)?;
    Ok(ShaderModule {
        name: baseline.name.clone(),
        span: baseline.span.clone(),
        origins: vec![],
        wgsl,
        naga,
    })
}

fn depth_record(wgsl: &str) -> Result<String, Failure> {
    // Camera D32 is normally discarded. Mirror its f32 output in the auxiliary visibility word,
    // using raster position.z for the shell. The diagnostic schedule runs without Shade.
    let depth = wgsl
        .replace(
            "bitcast<u32>(t)), clip.z / clip.w",
            "bitcast<u32>(clip.z / clip.w)), clip.z / clip.w",
        )
        .replace("q.x | (q.y << 16u)", "bitcast<u32>(v.pos.z)")
        .replace("bitcast<u32>(t)), depth", "bitcast<u32>(depth)), depth");
    if depth == wgsl {
        return Err(Failure::Compile("camera depth output ABI changed".into()));
    }
    Ok(depth)
}

fn program(fixture: Fixture) -> Result<Program, Failure> {
    let (path, set) = fixture.scene();
    let src = sources(Path::new(path)).map_err(Failure::Usage)?;
    let set = set
        .iter()
        .map(|&(name, value)| (name.to_owned(), value))
        .collect::<Vec<_>>();
    let p = loft_passes::compile(&src, &set)
        .map_err(|e| Failure::Compile(e.render(&src)))?
        .0;
    let ids = p
        .passes
        .iter()
        .find(|pass| fixture.subject(pass))
        .and_then(|pass| match pass {
            Pass::Shade(ids) | Pass::Vis(ids) => Some(ids),
            Pass::Shadow(_) | Pass::Authored(_) | Pass::Path(_) => None,
        })
        .ok_or_else(|| Failure::Compile("missing timed pass".into()))?;
    if ids.len() != fixture.kinds().len()
        || fixture.kinds().iter().any(|&(name, words, _, _)| {
            !ids.iter().any(|&id| {
                let k = &p.kinds[id as usize];
                k.name == name && k.stride == words
            })
        })
    {
        return Err(Failure::Compile(format!(
            "{} kind or row ABI changed",
            fixture.name()
        )));
    }
    Ok(p)
}

fn clone_kind(kind: &loft_passes::program::KindTable) -> loft_passes::program::KindTable {
    loft_passes::program::KindTable {
        name: kind.name.clone(),
        module: kind.module,
        stride: kind.stride,
        rows: kind.rows.clone(),
        n_rows: kind.n_rows,
        lipschitz: kind.lipschitz.clone(),
        draw: kind.draw,
        reads: kind.reads.clone(),
    }
}

fn replace_pass(p: &mut Program, fixture: Fixture, depth: bool) -> Result<(), Failure> {
    for &(name, _, vertex, entry) in fixture.kinds() {
        let kind = p
            .kinds
            .iter()
            .position(|k| k.name == name)
            .expect("checked kind ABI");
        let module = p.kinds[kind].module;
        let replacement = joined(fixture, &p.modules[module], vertex, entry, depth)?;
        if fixture == Fixture::Shrine {
            // Native uses one raster pipeline per kind. Give the sun a shadow-only alias of
            // each shared kind so its generated mesh/fragment pipeline survives the camera swap.
            for pass in &mut p.passes {
                if let Pass::Shadow(ids) = pass {
                    if let Some(id) = ids.iter_mut().find(|&&mut id| id as usize == kind) {
                        *id = p.kinds.len() as u32;
                        p.kinds.push(clone_kind(&p.kinds[kind]));
                        // The host fits the sun camera from instance bounds. An alias needs a
                        // bounds record, even though its camera indirect draw is never used.
                        let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                        for row in p.instances.iter().filter(|row| row.kind == kind as u32) {
                            for axis in 0..3 {
                                lo[axis] = lo[axis].min(row.lo[axis]);
                                hi[axis] = hi[axis].max(row.hi[axis]);
                            }
                        }
                        p.instances.push(loft_passes::program::Instance {
                            // Camera cull sees this as another bounded instance. Its alias
                            // draws only from the sun's independent identity index buffer.
                            lo,
                            hi,
                            kind: *id,
                            row: 0,
                        });
                    }
                }
            }
            p.kinds[kind].module = p.modules.len();
            p.modules.push(replacement);
        } else {
            p.modules[module] = replacement;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Capture {
    Beauty,
    Words,
    Depth,
}

fn load(g: &Gpu, p: &Program, capture: Capture) -> Result<Loaded, Failure> {
    let diagnostics = match capture {
        Capture::Beauty => Aov::Beauty,
        Capture::Words | Capture::Depth => Aov::Depth,
    };
    Loaded::with_aov(g, p, SIZE, None, diagnostics).map_err(Failure::Device)
}

fn diagnostic(p: &mut Program, capture: Capture) -> Result<(), Failure> {
    if matches!(capture, Capture::Depth) {
        p.shadow_size = 0;
        p.passes
            .retain(|pass| matches!(pass, Pass::Vis(_)) || pass.presents() || matches!(pass, Pass::Authored(a) if a.label == "cull"));
    }
    let present = p.module_named("present").ok_or_else(|| Failure::Compile("missing present module".into()))?;
    let present = &mut p.modules[present];
    present.naga = parse(READBACK)?;
    present.wgsl = READBACK.to_owned();
    Ok(())
}

fn snapshot(g: &Gpu, scene: &Loaded, t: f32, word: u32) -> Result<Vec<u8>, Failure> {
    scene.frame_samples(g, t, word);
    scene.pixels(g).map_err(Failure::Device)
}

#[derive(Serialize)]
struct Difference {
    name: &'static str,
    frame: usize,
    time_bits: u32,
    pixels_different: usize,
    pixels_over_1_ulp: usize,
    max_ulp: u32,
    by_kind: [usize; 6],
    examples: Vec<(usize, String, String)>,
    loft_sha256: String,
    hand_sha256: String,
}

#[derive(Clone, Copy)]
enum Bits {
    Rgba,
    Id,
    Word,
    Float32,
    Float16,
}

fn ordered(x: u32, sign: u32) -> u32 {
    if x & sign == 0 {
        sign + x
    } else {
        sign - (x & !sign)
    }
}

fn compare(
    name: &'static str,
    frame: usize,
    t: f32,
    a: &[u8],
    b: &[u8],
    bits: Bits,
    ids: &[u8],
) -> Difference {
    assert_eq!(a.len(), b.len());
    let mut row = Difference {
        name,
        frame,
        time_bits: t.to_bits(),
        pixels_different: 0,
        pixels_over_1_ulp: 0,
        max_ulp: 0,
        by_kind: [0; 6],
        examples: vec![],
        loft_sha256: digest(a),
        hand_sha256: digest(b),
    };
    for (index, (a, b)) in a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .enumerate()
    {
        if a == b {
            continue;
        }
        let ulp = match bits {
            Bits::Rgba => a
                .iter()
                .zip(b)
                .map(|(a, b)| u32::from(a.abs_diff(*b)))
                .max()
                .unwrap(),
            Bits::Id | Bits::Word => u32::MAX,
            Bits::Float32 => ordered(u32::from_le_bytes(*a), 1 << 31)
                .abs_diff(ordered(u32::from_le_bytes(*b), 1 << 31)),
            Bits::Float16 => (0..2)
                .map(|i| {
                    let half =
                        |p: &[u8; 4]| u32::from(u16::from_le_bytes([p[i * 2], p[i * 2 + 1]]));
                    ordered(half(a), 1 << 15).abs_diff(ordered(half(b), 1 << 15))
                })
                .max()
                .unwrap(),
        };
        row.pixels_different += 1;
        row.pixels_over_1_ulp += usize::from(ulp > 1);
        row.max_ulp = row.max_ulp.max(ulp);
        let kind = usize::from(ids[index * 4 + 3]).min(5);
        row.by_kind[kind] += 1;
        if row.examples.len() < 5 {
            row.examples.push((
                index,
                format!("{:08x}", u32::from_le_bytes(*a)),
                format!("{:08x}", u32::from_le_bytes(*b)),
            ));
        }
    }
    row
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn pin_cull_order(p: &mut Program) -> Result<(), Failure> {
    // Compact atomic appends can change the winning row at equal D32 depths.
    // The compiler's indexed cull fixes draw order for diagnostic comparisons.
    let cull = p.module_named("cull").ok_or_else(|| Failure::Compile("missing cull module".into()))?;
    let m = &mut p.modules[cull];
    // The stdlib cull fills its `draw` hole with one of these two fragments; swap compact for indexed.
    let (compact, indexed) = (include_str!("../../../stdlib/core/cull_multi.wgsl"), include_str!("../../../stdlib/core/cull_single.wgsl"));
    if m.wgsl.contains(compact) { m.wgsl = m.wgsl.replace(compact, indexed); }
    m.naga = parse(&m.wgsl)?;
    Ok(())
}

fn parity(g: &Gpu, fixture: Fixture, mutant: bool) -> Result<Vec<Difference>, Failure> {
    let cases = match fixture {
        Fixture::Hermit => vec![0, fixture.frames() / 2, fixture.frames() - 1],
        Fixture::Shrine => vec![0],
    };
    let mut pa = program(fixture)?;
    let mut pb = program(fixture)?;
    pin_cull_order(&mut pa)?;
    if mutant {
        add_mutant(&mut pa, fixture);
    }
    replace_pass(&mut pb, fixture, false)?;
    pin_cull_order(&mut pb)?;
    let a = load(g, &pa, Capture::Beauty)?;
    let b = load(g, &pb, Capture::Beauty)?;
    let mut report = Vec::new();
    for capture in [Capture::Words, Capture::Depth] {
        let mut pa = program(fixture)?;
        let mut pb = program(fixture)?;
        if matches!(capture, Capture::Depth) {
            let vis = pa
                .passes
                .iter()
                .find_map(|pass| {
                    if let Pass::Vis(ids) = pass {
                        Some(ids)
                    } else {
                        None
                    }
                })
                .unwrap();
            for &id in vis {
                let m = &mut pa.modules[pa.kinds[id as usize].module];
                m.wgsl = depth_record(&m.wgsl)?;
                m.naga = parse(&m.wgsl)?;
            }
            if fixture == Fixture::Hermit {
                let vis = pb
                    .passes
                    .iter()
                    .find_map(|pass| {
                        if let Pass::Vis(ids) = pass {
                            Some(ids)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                for &id in vis {
                    let m = &mut pb.modules[pb.kinds[id as usize].module];
                    m.wgsl = depth_record(&m.wgsl)?;
                    m.naga = parse(&m.wgsl)?;
                }
            }
        }
        diagnostic(&mut pa, capture)?;
        diagnostic(&mut pb, capture)?;
        replace_pass(
            &mut pb,
            fixture,
            matches!((fixture, capture), (Fixture::Shrine, Capture::Depth)),
        )?;
        pin_cull_order(&mut pa)?;
        pin_cull_order(&mut pb)?;
        let da = load(g, &pa, capture)?;
        let db = load(g, &pb, capture)?;
        for &frame in &cases {
            let t = fixture.time(frame);
            let (ids_a, ids_b) = (snapshot(g, &da, t, 0)?, snapshot(g, &db, t, 0)?);
            match capture {
                Capture::Words => {
                    report.push(compare("id", frame, t, &ids_a, &ids_b, Bits::Id, &ids_a));
                    let images = (snapshot(g, &a, t, 1)?, snapshot(g, &b, t, 1)?);
                    report.push(compare(
                        "rgba8",
                        frame,
                        t,
                        &images.0,
                        &images.1,
                        Bits::Rgba,
                        &ids_a,
                    ));
                    let vis_bits = if fixture == Fixture::Shrine {
                        Bits::Float32
                    } else {
                        Bits::Word
                    };
                    for (word, name, bits) in [
                        (1, "visibility_aux", vis_bits),
                        (2, "hdr_rg", Bits::Float16),
                        (3, "hdr_ba", Bits::Float16),
                    ] {
                        let (a, b) = (snapshot(g, &da, t, word)?, snapshot(g, &db, t, word)?);
                        report.push(compare(name, frame, t, &a, &b, bits, &ids_a));
                    }
                }
                Capture::Depth => {
                    let (a, b) = (snapshot(g, &da, t, 4)?, snapshot(g, &db, t, 4)?);
                    report.push(compare("depth32", frame, t, &a, &b, Bits::Float32, &ids_a));
                }
                Capture::Beauty => {
                    unreachable!("beauty readback uses the original present pipeline")
                }
            }
        }
    }
    Ok(report)
}

fn math_directives(source: &str) -> usize {
    source
        .lines()
        .filter(|line| {
            let normalized = line
                .to_ascii_lowercase()
                .split_whitespace()
                .collect::<String>();
            normalized.contains("contract(off)")
                || normalized.contains("mtlmathmodesafe")
                || normalized.contains("mathmode::safe")
                || normalized.contains("setfastmathenabled(false)")
        })
        .count()
}

#[cfg(target_os = "macos")]
fn emitted_msl(module: &ShaderModule, entry: &str) -> Result<String, Failure> {
    use naga::back::msl::{self, Options, PipelineOptions};
    let stage = module
        .naga
        .entry_points
        .iter()
        .find(|e| e.name == entry)
        .ok_or_else(|| Failure::Compile(format!("missing shader entry {entry}")))?
        .stage;
    let info = validate(&module.naga, &module.wgsl)?;
    let options = Options {
        lang_version: (3, 2),
        ..Options::default()
    };
    let pipeline = PipelineOptions {
        entry_point: Some((stage, entry.to_owned())),
        ..PipelineOptions::default()
    };
    msl::write_string(&module.naga, &info, &options, &pipeline)
        .map(|(source, _)| source)
        .map_err(|e| {
            Failure::Compile(format!(
                "{} {entry} MSL policy inspection: {e}",
                module.name
            ))
        })
}

#[derive(Serialize)]
struct MathPolicy {
    kind: String,
    entry: String,
    wgsl_sha256: String,
    wgsl_forbidden_directives: usize,
    msl_sha256: Option<String>,
    msl_forbidden_directives: Option<usize>,
}

fn guard_math(p: &Program, fixture: Fixture) -> Result<Vec<MathPolicy>, Failure> {
    let mut result = Vec::new();
    for &(name, _, _, _) in fixture.kinds() {
        let k = p
            .kinds
            .iter()
            .find(|k| k.name == name)
            .expect("checked kind ABI");
        let module = &p.modules[k.module];
        let entries: &[&str] = match fixture {
            Fixture::Hermit => &["shade"],
            Fixture::Shrine => &["vs", "fs"],
        };
        for &entry in entries {
            #[cfg(target_os = "macos")]
            let msl = Some(emitted_msl(module, entry)?);
            #[cfg(not(target_os = "macos"))]
            let msl: Option<String> = None;
            let wgsl_forbidden_directives = math_directives(&module.wgsl);
            let msl_forbidden_directives = msl.as_deref().map(math_directives);
            if wgsl_forbidden_directives > 0 || msl_forbidden_directives.is_some_and(|n| n > 0) {
                return Err(Failure::Compile(format!(
                    "{name} {entry}: forbidden contraction or safe-math directive"
                )));
            }
            result.push(MathPolicy {
                kind: name.into(),
                entry: entry.into(),
                wgsl_sha256: digest(module.wgsl.as_bytes()),
                wgsl_forbidden_directives,
                msl_forbidden_directives,
                msl_sha256: msl.map(|s| digest(s.as_bytes())),
            });
        }
    }
    Ok(result)
}

fn fma_witness(g: &Gpu) -> Result<u32, Failure> {
    use wgpu::{BufferUsages as U, util::DeviceExt as _};
    const SOURCE: &str = "
@group(0) @binding(0) var<storage, read_write> words: array<f32>;
@compute @workgroup_size(1) fn witness() {
    words[3] = words[0] * words[1] + words[2];
}";
    let d = &g.device;
    let words = [0x3f80_0001u32, 0x3f7f_fffe, 0xbf80_0000, 0].map(u32::to_le_bytes);
    let buffer = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("contraction witness inputs"),
        contents: words.as_flattened(),
        usage: U::STORAGE | U::COPY_SRC,
    });
    let read = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("contraction witness result"),
        size: 4,
        usage: U::COPY_DST | U::MAP_READ,
        mapped_at_creation: false,
    });
    let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("contraction witness"),
        source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(parse(SOURCE)?)),
    });
    let pipeline = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("witness"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    let mut encoder = d.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&buffer, 12, &read, 0, Some(4));
    g.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    read.map_async(wgpu::MapMode::Read, .., move |r| {
        let _ = tx.send(r);
    });
    d.poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| Failure::Device(e.to_string()))?;
    rx.try_recv()
        .map_err(|e| Failure::Device(e.to_string()))?
        .map_err(|e| Failure::Device(e.to_string()))?;
    let view = read
        .get_mapped_range(..)
        .map_err(|e| Failure::Device(e.to_string()))?;
    Ok(u32::from_le_bytes(view[..4].try_into().unwrap()))
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
enum Arm {
    Loft,
    Hand,
}

#[derive(Serialize)]
struct TimingBlock {
    arm: Arm,
    passes: Vec<String>,
    load_control_before_p50_ms: f64,
    load_control_after_p50_ms: f64,
    frame_time_bits: Vec<u32>,
    frames: Vec<FrameTiming>,
    subject_ms: Vec<f64>,
    whole_frame_ms: Vec<f64>,
}

impl TimingBlock {
    fn contended(&self) -> bool {
        self.load_control_before_p50_ms > QUIET_CONTROL_P50_MS
            || self.load_control_after_p50_ms > QUIET_CONTROL_P50_MS
    }
}

fn whole_frame_ticks(ticks: &[u64]) -> Option<u64> {
    // Reused static shadows occupy a slot in pass order with no timestamps.
    let mut pairs = ticks.chunks_exact(2).filter(|pair| *pair != [0, 0]);
    let pair = pairs.next()?;
    let (first, last) = pairs.fold((pair[0], pair[1]), |(first, last), pair| {
        (first.min(pair[0]), last.max(pair[1]))
    });
    Some(last - first)
}

fn block(
    g: &Gpu,
    f: Fixture,
    arm: Arm,
    scene: &Loaded,
    p: &Program,
) -> Result<TimingBlock, Failure> {
    let times = (0..f.frames()).map(|i| f.time(i)).collect::<Vec<_>>();
    let warmup = (0..WARMUP).map(|i| match f {
        Fixture::Hermit => i as f32 / 30.0,
        Fixture::Shrine => 0.0,
    });
    let [load_control_before_p50_ms, _] = g.load_control(SIZE).map_err(Failure::Device)?;
    let frames = scene
        .profile_frames(g, warmup.chain(times.iter().copied()), true)
        .map_err(Failure::Device)?
        .into_iter()
        .skip(WARMUP)
        .collect::<Vec<_>>();
    let [load_control_after_p50_ms, _] = g.load_control(SIZE).map_err(Failure::Device)?;
    let period = f64::from(g.queue.get_timestamp_period()) * 1e-6;
    let indices = p
        .passes
        .iter()
        .enumerate()
        .filter_map(|(i, pass)| f.subject(pass).then_some(i))
        .collect::<Vec<_>>();
    let mut subject_ms = Vec::with_capacity(frames.len());
    let mut whole_frame_ms = Vec::with_capacity(frames.len());
    for frame in &frames {
        if frame.ticks.len() != p.passes.len() * 2
            || frame.ticks.chunks_exact(2).any(|x| x[1] < x[0])
        {
            return Err(Failure::Device(format!(
                "frame {} has invalid pass stamps {:?}",
                frame.serial, frame.ticks
            )));
        }
        let elapsed = |first, last| (last - first) as f64 * period;
        let duration = indices
            .iter()
            .map(|&i| elapsed(frame.ticks[2 * i], frame.ticks[2 * i + 1]))
            .sum();
        subject_ms.push(duration);
        let ticks = whole_frame_ticks(&frame.ticks)
            .ok_or_else(|| Failure::Device(format!("frame {} has no GPU pass stamps", frame.serial)))?;
        whole_frame_ms.push(ticks as f64 * period);
    }
    Ok(TimingBlock {
        arm,
        passes: p.passes.iter().map(|pass| format!("{pass:?}")).collect(),
        load_control_before_p50_ms,
        load_control_after_p50_ms,
        frame_time_bits: times.into_iter().map(f32::to_bits).collect(),
        frames,
        subject_ms,
        whole_frame_ms,
    })
}

fn quantile(data: &mut [f64], numerator: usize, denominator: usize) -> f64 {
    let rank = (data.len() * numerator).div_ceil(denominator) - 1;
    *data.select_nth_unstable_by(rank, f64::total_cmp).1
}

#[derive(Serialize)]
struct Quantiles {
    p50_ms: f64,
    p95_ms: f64,
}

impl Quantiles {
    fn of(data: &mut [f64]) -> Self {
        Self {
            p50_ms: quantile(data, 50, 100),
            p95_ms: quantile(data, 95, 100),
        }
    }
}

#[derive(Serialize)]
struct Ratio {
    loft: Quantiles,
    hand: Quantiles,
    ratio: f64,
    ci_low: f64,
    ci_high: f64,
}

fn bootstrap(
    blocks: &[TimingBlock],
    metric: impl Fn(&TimingBlock) -> &[f64],
) -> Result<Ratio, Failure> {
    let group = |arm| {
        blocks
            .iter()
            .filter(|block| block.arm == arm)
            .map(&metric)
            .collect::<Vec<_>>()
    };
    let a = group(Arm::Loft);
    let b = group(Arm::Hand);
    let n = a[0].len();
    let flatten = |blocks: &[&[f64]]| {
        blocks
            .iter()
            .flat_map(|samples| samples.iter().copied())
            .collect::<Vec<_>>()
    };
    let loft = Quantiles::of(&mut flatten(&a));
    let hand = Quantiles::of(&mut flatten(&b));
    let mut state = 0x57e6_b2a9_946d_481du64;
    let mut samples = Vec::with_capacity(BOOTSTRAPS);
    let (mut draw_a, mut draw_b) = (Vec::with_capacity(2 * n), Vec::with_capacity(2 * n));
    for _ in 0..BOOTSTRAPS {
        draw_a.clear();
        draw_b.clear();
        for offset in (0..n).step_by(BLOCK) {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let start = state as usize % n;
            for j in 0..BLOCK.min(n - offset) {
                let index = (start + j) % n;
                draw_a.extend([a[0][index], a[1][index]]);
                draw_b.extend([b[0][index], b[1][index]]);
            }
        }
        samples.push(quantile(&mut draw_a, 95, 100) / quantile(&mut draw_b, 95, 100));
    }
    let ratio = loft.p95_ms / hand.p95_ms;
    let ci_low = quantile(&mut samples, 25, 1000);
    let ci_high = quantile(&mut samples, 975, 1000);
    if ![ratio, ci_low, ci_high]
        .into_iter()
        .all(|x| x.is_finite() && x > 0.0)
    {
        return Err(Failure::Device(
            "invalid p95 ratio from GPU timestamps".into(),
        ));
    }
    Ok(Ratio {
        loft,
        hand,
        ratio,
        ci_low,
        ci_high,
    })
}

fn add_mutant(p: &mut Program, f: Fixture) {
    let index = p
        .passes
        .iter()
        .position(|pass| f.subject(pass))
        .expect("checked pass");
    match &mut p.passes[index] {
        Pass::Shade(ids) => ids.extend_from_within(..),
        // A second complete render clears the depth/color and executes the same fragment work.
        Pass::Vis(ids) => {
            let repeat = Pass::Vis(ids.clone());
            p.passes.insert(index + 1, repeat);
        }
        Pass::Shadow(_) | Pass::Authored(_) | Pass::Path(_) => {
            unreachable!("checked pass")
        }
    }
}

struct Measurement {
    pass: Ratio,
    contended: bool,
    parity_held: bool,
}

fn execute(
    g: &Gpu,
    f: Fixture,
    mutant: bool,
    reverse: bool,
    out: &Path,
    witness: u32,
) -> Result<Measurement, Failure> {
    let start = Instant::now();
    let mut a = program(f)?;
    let mut b = program(f)?;
    replace_pass(&mut b, f, false)?;
    let math = guard_math(&a, f)?;
    if mutant {
        add_mutant(&mut a, f);
    }
    let aa = load(g, &a, Capture::Beauty)?;
    let bb = load(g, &b, Capture::Beauty)?;
    let parity = parity(g, f, mutant)?;
    let parity_held = parity.iter().all(|x| match x.name {
        "hdr_rg" | "hdr_ba" if f == Fixture::Hermit => x.pixels_over_1_ulp == 0,
        _ => x.pixels_different == 0,
    });
    let prefix = format!(
        "{}{}{}",
        f.name(),
        if mutant { "-mutant" } else { "" },
        if reverse { "-baab" } else { "" }
    );
    let write = |name, value: &serde_json::Value| {
        let path = out.join(format!("{prefix}-{name}.json"));
        write_json(&path.to_string_lossy(), value)
    };
    write(
        "parity",
        &serde_json::json!({ "held": parity_held, "attachments": parity }),
    )?;
    let mut order = [Arm::Loft, Arm::Hand, Arm::Hand, Arm::Loft];
    if reverse {
        order = [Arm::Hand, Arm::Loft, Arm::Loft, Arm::Hand];
    }
    let mut blocks = Vec::new();
    for arm in order {
        let (loaded, p) = match arm {
            Arm::Loft => (&aa, &a),
            Arm::Hand => (&bb, &b),
        };
        blocks.push(block(g, f, arm, loaded, p)?);
    }
    write(
        "abba",
        &serde_json::json!({
            "adapter": g.device_signature, "size": SIZE, "fixture": f.name(), "mutant": mutant, "warmups_per_block": WARMUP,
            "scene": f.scene(), "timestamp_period_ns": g.queue.get_timestamp_period(), "blocks": blocks,
            "control_sha256": digest(f.source().as_bytes()), "loft_math": math, "fma_witness_bits": format!("{witness:08x}"),
        }),
    )?;
    let pass = bootstrap(&blocks, |b| &b.subject_ms)?;
    let frame = bootstrap(&blocks, |b| &b.whole_frame_ms)?;
    let contended = blocks.iter().any(TimingBlock::contended);
    let slower = pass.ci_low > LIMIT;
    let point_flag = pass.ratio > LIMIT;
    write(
        "stats",
        &serde_json::json!({
            "fixture": f.name(), "mutant": mutant, "pass": pass, "frame": frame, "limit": LIMIT,
            "contended": contended, "quiet_control_p50_ms": QUIET_CONTROL_P50_MS, "parity_held": parity_held,
            "measurably_slower": slower, "point_estimate_over_limit": point_flag,
            "interval_crosses_limit": pass.ci_low <= LIMIT && pass.ci_high > LIMIT,
            "elapsed_seconds": start.elapsed().as_secs_f64(), "bootstrap_block_frames": BLOCK,
            "bootstrap_replicates": BOOTSTRAPS,
        }),
    )?;
    let label = if mutant { " mutant" } else { "" };
    println!(
        "{}{label}: p95 ratio {:.3}x (95% CI {:.3}–{:.3}), frame {:.3}x, parity {}, load control p50 before/after {:?}{}",
        f.name(),
        pass.ratio,
        pass.ci_low,
        pass.ci_high,
        frame.ratio,
        if parity_held { "held" } else { "miss" },
        blocks.iter().map(|b| [b.load_control_before_p50_ms, b.load_control_after_p50_ms]).collect::<Vec<_>>(),
        if point_flag {
            ", point estimate exceeds 1.05x"
        } else {
            ""
        }
    );
    Ok(Measurement {
        pass,
        contended,
        parity_held,
    })
}

fn score(
    g: &Gpu,
    f: Fixture,
    mutant: bool,
    reverse: bool,
    out: &Path,
    witness: u32,
) -> Result<(), Failure> {
    let label = if mutant { " mutant" } else { "" };
    let name = f.name();
    let mut measurement = execute(g, f, mutant, reverse, out, witness)?;
    // Retest only a usable output comparison, and keep both launch orders in the evidence.
    if measurement.parity_held
        && (measurement.contended
            || (measurement.pass.ci_low <= LIMIT && measurement.pass.ci_high > LIMIT))
    {
        println!("{name}{label}: repeating in reversed order");
        measurement = execute(g, f, mutant, !reverse, out, witness)?;
    }
    measurement.verdict(f, mutant)
}

impl Measurement {
    fn verdict(self, fixture: Fixture, mutant: bool) -> Result<(), Failure> {
        let name = fixture.name();
        let label = if mutant { " mutant" } else { "" };
        let Self {
            pass,
            contended,
            parity_held,
        } = self;
        if contended {
            return Err(Failure::Device(format!(
                "{name}{label}: load control p50 exceeds {QUIET_CONTROL_P50_MS:.2} ms before or after a block"
            )));
        }
        if pass.ci_low > LIMIT {
            return Err(Failure::Budget(format!(
                "{name}{label}: {:.3}x p95, lower 95% bound {:.3}x exceeds {LIMIT:.2}x",
                pass.ratio, pass.ci_low
            )));
        }
        if !parity_held {
            return Err(Failure::Parity(format!(
                "{name}{label}: attachment parity failed"
            )));
        }
        if pass.ci_high > LIMIT {
            return Err(Failure::Device(format!(
                "{name}{label}: inconclusive after both orders, 95% interval {:.3}x–{:.3}x crosses {LIMIT:.2}x",
                pass.ci_low, pass.ci_high
            )));
        }
        if mutant {
            return Err(Failure::Truth(format!(
                "{name}{label}: slowdown did not cross the threshold"
            )));
        }
        Ok(())
    }
}

pub(crate) fn run(args: &[String], set: &[(String, f64)]) -> Result<(), Failure> {
    let mut args = args.to_vec();
    let out = flag(&mut args, "--out")?.unwrap_or_else(|| "out/meta/s1_controls".into());
    let only = flag(&mut args, "--only")?;
    let fixtures = match only.as_deref() {
        None => &[Fixture::Hermit, Fixture::Shrine][..],
        Some("hermit_shade") => &[Fixture::Hermit],
        Some("shrine_vis") => &[Fixture::Shrine],
        Some(name) => return Err(Failure::Usage(format!("unknown control {name}"))),
    };
    let take = |args: &mut Vec<String>, flag| {
        args.iter()
            .position(|x| x == flag)
            .map(|i| args.remove(i))
            .is_some()
    };
    let mutant = take(&mut args, "--mutant");
    let reverse = take(&mut args, "--reverse");
    let validate_only = take(&mut args, "--validate");
    if !set.is_empty() || !args.is_empty() {
        return Err(Failure::Usage("usage: loft controls [--only hermit_shade|shrine_vis] [--mutant] [--reverse] [--validate] [--out DIR]".into()));
    }
    parse(READBACK)?;
    for &fixture in fixtures {
        parse(fixture.source())?;
        let mut p = program(fixture)?;
        guard_math(&p, fixture)?;
        replace_pass(&mut p, fixture, false)?;
        println!("{}: Naga validation held", fixture.name());
    }
    if validate_only {
        return Ok(());
    }
    let g = native::gpu().map_err(Failure::Device)?;
    budget::timestamps(g.timestamps)?;
    let witness = fma_witness(&g)?;
    if g.adapter.ends_with("(Metal)") && witness != 0xa880_0000 {
        return Err(Failure::Device(format!(
            "Metal contraction witness returned {witness:08x}, expected a8800000"
        )));
    }
    let out = Path::new(&out);
    std::fs::create_dir_all(out).map_err(|e| Failure::Usage(format!("{}: {e}", out.display())))?;
    let mut error = None;
    for &fixture in fixtures {
        if let Err(e) = score(&g, fixture, mutant, reverse, out, witness) {
            // A measured budget miss retains exit 7 when another fixture also fails.
            if error.is_none() || matches!(e, Failure::Budget(_)) {
                if let Some(previous) = error.replace(e) {
                    eprintln!("{}", previous.exit().1);
                }
            } else {
                eprintln!("{}", e.exit().1);
            }
        }
    }
    error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_span_ignores_skipped_shadow_stamps() {
        assert_eq!(whole_frame_ticks(&[1000, 1100, 0, 0, 1080, 1350, 1200, 1280]), Some(350));
        assert_eq!(whole_frame_ticks(&[0, 0]), None);
    }

    #[test]
    fn only_a_conclusive_slowdown_uses_the_budget_exit() {
        let verdict = |ci_low, ci_high, contended, parity_held| {
            let ratio = (ci_low + ci_high) / 2.0;
            Measurement {
                pass: Ratio {
                    loft: Quantiles {
                        p50_ms: 1.0,
                        p95_ms: ratio,
                    },
                    hand: Quantiles {
                        p50_ms: 1.0,
                        p95_ms: 1.0,
                    },
                    ratio,
                    ci_low,
                    ci_high,
                },
                contended,
                parity_held,
            }
            .verdict(Fixture::Shrine, false)
            .err()
            .map(|e| e.exit().0)
        };
        assert_eq!(verdict(1.04, 1.05, false, true), None);
        assert_eq!(verdict(1.04, 1.06, false, true), Some(6));
        assert_eq!(verdict(1.06, 1.07, false, true), Some(7));
        assert_eq!(verdict(1.06, 1.07, true, true), Some(6));
        assert_eq!(verdict(1.04, 1.05, false, false), Some(3));
    }

    #[test]
    fn bootstrap_resamples_replay_time_jointly_across_all_four_blocks() {
        let blocks = [Arm::Loft, Arm::Hand, Arm::Hand, Arm::Loft].map(|arm| {
            let scale = if arm == Arm::Loft { 2.0 } else { 1.0 };
            TimingBlock {
                arm,
                passes: vec![],
                load_control_before_p50_ms: 0.0,
                load_control_after_p50_ms: 0.0,
                frame_time_bits: vec![],
                frames: vec![],
                subject_ms: (1..=100).map(|i| f64::from(i * i) * scale).collect(),
                whole_frame_ms: vec![],
            }
        });
        let result = bootstrap(&blocks, |b| &b.subject_ms).unwrap();
        assert_eq!([result.ratio, result.ci_low, result.ci_high], [2.0; 3]);
    }
}
