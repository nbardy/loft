//! The five gates (plan 2026-11-20 §4), live on every scene in `engines/gates.json`:
//!
//!   pictures  every expected frame renders and matches its approved reference (a missing frame or reference fails)
//!   twin      the GPU's queries equal the CPU evaluator's within the tier contract
//!   speed     p95 GPU frame time and GPU commands per frame within the scene's budget
//!   load      modules, pipelines, cold and warm load within budget; modules identical across a scene's instance counts
//!   truth     what the compiler claims (derived L, derived bounds, `visible` agreeing with `trace`) sampled: Held
//!
//! `--approve` writes the current renders as references (and their hashes into gates.json), then gates strictly.
//! Every check is a `Verdict`; a gate passes only when all of its verdicts are `Held`.

use crate::budget::{self, Failure};
use loft_ir::Verdict;
use loft_passes::program::{Draw, Program};
use loft_passes::Compiled;
use loft_cli::native::{Gpu, Loaded};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gate {
    Pictures,
    Twin,
    Speed,
    Load,
    Truth,
}

const GATES: [Gate; 5] = [Gate::Pictures, Gate::Twin, Gate::Speed, Gate::Load, Gate::Truth];

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    size: (u32, u32),
    frames: Vec<f32>,
    timing_frames: usize,
    #[serde(deserialize_with = "unique_map")]
    refs: BTreeMap<String, String>,
    scenes: Vec<Scene>,
}

fn unique_map<'de, D, V>(d: D) -> Result<BTreeMap<String, V>, D::Error>
where D: serde::Deserializer<'de>, V: Deserialize<'de> {
    struct Visitor<V>(std::marker::PhantomData<V>);
    impl<'de, V: Deserialize<'de>> serde::de::Visitor<'de> for Visitor<V> {
        type Value = BTreeMap<String, V>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an object with unique keys")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            while let Some((k, v)) = a.next_entry::<String, V>()? {
                if result.insert(k.clone(), v).is_some() {
                    return Err(serde::de::Error::custom(format!("duplicate key {k}")));
                }
            }
            Ok(result)
        }
    }
    d.deserialize_map(Visitor(std::marker::PhantomData))
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Scene {
    name: String,
    file: PathBuf,
    #[serde(deserialize_with = "unique_map")]
    set: BTreeMap<String, f64>,
    budget: Budget,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Budget {
    p95_ms: f64,
    commands: u32,
    modules: u32,
    pipelines: u32,
    shapes: usize,
    cold_s: f64,
    warm_s: f64,
}

impl Manifest {
    fn select(&self, only: Option<&str>, approve: bool) -> Result<Vec<&Scene>, Failure> {
        let mut names = BTreeSet::new();
        let mut times = BTreeSet::new();
        if self.size.0 == 0 || self.size.1 == 0 || self.timing_frames == 0
            || self.scenes.is_empty() || self.frames.is_empty()
            || self.scenes.iter().any(|s| s.name.is_empty() || !names.insert(s.name.as_str()))
            || self.frames.iter().any(|t| !t.is_finite() || !times.insert(if *t == 0.0 { 0 } else { t.to_bits() }))
        {
            return Err(Failure::Parity("gates.json: invalid size or empty, duplicate, or invalid scene/frame list".into()));
        }
        let mut selected = BTreeSet::new();
        if let Some(only) = only {
            for name in only.split(',') {
                if !names.contains(name) || !selected.insert(name) {
                    return Err(Failure::Usage(format!("invalid or duplicate gate selection {name:?}")));
                }
            }
        }
        let scenes: Vec<_> = self.scenes.iter().filter(|s| selected.is_empty() || selected.contains(s.name.as_str())).collect();
        let expected: BTreeSet<_> = scenes.iter().flat_map(|s| self.frames.iter().map(move |t| format!("{}@{t}", s.name))).collect();
        let actual: BTreeSet<_> = self.refs.keys().filter(|key| selected.is_empty() || key.split_once('@').is_some_and(|(s, _)| selected.contains(s))).cloned().collect();
        if !approve && expected != actual {
            return Err(Failure::Parity(format!("gates.json: approved frame keys differ. Missing {:?}, extra {:?}", expected.difference(&actual), actual.difference(&expected))));
        }
        Ok(scenes)
    }
}

struct Check {
    gate: Gate,
    scene: String,
    what: String,
    verdict: Verdict,
    contended: bool,
}

const TWIN_TOL: [f64; 5] = [1e-5, 1e-5, 2e-5, 1e-5, 1e-4];

fn held_if(slack: f64, unknown: Option<String>) -> Verdict {
    match unknown {
        Some(reason) => Verdict::Unknown { reason },
        None if !slack.is_finite() => Verdict::Unknown { reason: "missing or nonfinite evidence".into() },
        None if slack >= 0.0 => Verdict::Held { margin: slack },
        None => Verdict::Missed { by: -slack },
    }
}

#[derive(Default)]
struct Evidence {
    min: Option<f64>,
    invalid: bool,
}
impl Evidence {
    fn add(&mut self, margin: f64) {
        self.invalid |= !margin.is_finite();
        self.min = Some(self.min.map_or(margin, |old| old.min(margin)));
    }
    fn verdict(&self) -> Verdict {
        held_if(self.min.unwrap_or(f64::NAN), self.invalid.then(|| "nonfinite evidence".into()))
    }
}

fn twin_error(quantity: usize, cpu: f32, gpu: f32) -> f64 {
    if !cpu.is_finite() || !gpu.is_finite() { return f64::NAN; }
    // Two misses agree regardless of their negative sentinel.
    if quantity == 1 && (cpu < 0.0 || gpu < 0.0) {
        return if cpu < 0.0 && gpu < 0.0 { 0.0 } else { 1.0 };
    }
    (f64::from(cpu) - f64::from(gpu)).abs()
}

fn rgba_len(size: (u32, u32), pixels: &[u8]) -> bool {
    (size.0 as usize).checked_mul(size.1 as usize).and_then(|n| n.checked_mul(4)) == Some(pixels.len())
}

fn root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    cwd.ancestors()
        .find(|d| d.join("engines/gates.json").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| "run from inside the repository: no engines/gates.json above the current directory".into())
}

pub(crate) fn write_png(path: &Path, size: (u32, u32), rgba: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), size.0, size.1);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().and_then(|mut w| w.write_image_data(rgba)).map_err(|e| e.to_string())
}

fn read_png(path: &Path) -> Result<((u32, u32), Vec<u8>), String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut r = png::Decoder::new(std::io::BufReader::new(f)).read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("png too large")?];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    Ok(((info.width, info.height), buf))
}

fn decode_verdict(record: Option<[f32; 2]>) -> Verdict {
    match record {
        Some([1.0, value]) if value.is_finite() && value >= 0.0 => Verdict::Held { margin: value.into() },
        Some([2.0, value]) if value.is_finite() && value >= 0.0 => Verdict::Missed { by: value.into() },
        Some([3.0, _]) => Verdict::Unknown { reason: "authored check returned Unknown".into() },
        _ => Verdict::Unknown { reason: "missing or nonfinite check record".into() },
    }
}

pub(crate) fn check_records(p: &Program, samples: &[loft_passes::program::MotionSample]) -> Vec<(String, Verdict)> {
    let mut results = vec![];
    for step in &p.dispatches {
        let (kind, rows, _, _) = step.input.view(&p.kinds).expect("compiled dispatch input");
        for name in &step.checks {
            let mut records = samples.iter().filter(|s| s.kind == kind)
                .flat_map(|s| &s.checks).filter(|(n, _)| n == name).map(|(_, v)| v);
            let values = records.next().filter(|v| records.next().is_none() && v.len() == rows as usize);
            for row in 0..rows as usize {
                results.push((format!("{kind}[{row}].{name}"), decode_verdict(values.map(|v| v[row]))));
            }
        }
    }
    results
}

pub fn authored_checks(g: &Gpu, p: &Program, t: f32) -> Result<Vec<(String, Verdict)>, String> {
    if p.dispatches.iter().all(|m| m.checks.is_empty()) { return Ok(vec![]); }
    loft_cli::native::sample_motion(g, p, t).map(|samples| check_records(p, &samples))
}

pub fn still(scene: &Path, set: &[(String, f64)], t: f32, out: &Path, max_frame: Option<f64>) -> Result<(), Failure> {
    let src = crate::sources(scene).map_err(Failure::Usage)?;
    let (mut p, _, _) = loft_passes::compile(&src, set).map_err(|d| Failure::Compile(d.render(&src)))?;
    let g = loft_cli::native::gpu().map_err(Failure::Device)?;
    if max_frame.is_some() { budget::timestamps(g.timestamps)?; }
    if !p.dispatches.is_empty() {
        let sampled = loft_cli::native::sample_motion(&g, &p, t).map_err(Failure::Device)?;
        for kind in &sampled {
            for (n, values) in &kind.fields { println!("{}.{} {:?}", kind.kind, n, values); }
            for (n, values) in &kind.checks { println!("check {} {:?}", n, values); }
        }
        p = loft_passes::compile_snapshot(&src, set, &sampled).map_err(|d| Failure::Compile(d.render(&src)))?.0;
    }
    let size = (1280, 800);
    let l = Loaded::new(&g, &p, size, None).map_err(Failure::Device)?;
    l.frame(&g, t);
    if let Some(max) = max_frame { budget::check(&g, &l, max)?; }
    let px = l.pixels(&g).map_err(Failure::Device)?;
    write_png(out, size, &px).map_err(Failure::Usage)?;
    println!("wrote {}", out.display());
    Ok(())
}

/// Deterministic samples (a 64-bit LCG): the gates must not change between runs.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn in_box(&mut self, lo: [f32; 3], hi: [f32; 3]) -> [f32; 3] {
        [0, 1, 2].map(|i| lo[i] + (hi[i] - lo[i]) * self.next())
    }
    fn dir(&mut self) -> [f32; 3] {
        loop {
            let v = [0, 1, 2].map(|_| 2.0 * self.next() - 1.0);
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if (0.1..=1.0).contains(&l) {
                return v.map(|x| x / l);
            }
        }
    }
}

pub(crate) fn quantile(xs: &[f64], q: f64) -> f64 {
    let mut s = xs.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() - 1) as f64 * q).round() as usize]
}

struct Ctx<'a> {
    g: &'a Gpu,
    root: PathBuf,
    size: (u32, u32),
    frames: Vec<f32>,
    timing_frames: usize,
    approve: bool,
    refs: BTreeMap<String, String>,
    /// Module texts per scene file, to check identity across instance counts.
    modules: Vec<(PathBuf, String, Vec<String>)>,
}

/// One gate on one scene: its verdicts, plus numbers for the report.
#[allow(clippy::too_many_arguments)]
fn gate(c: &mut Ctx, gate: Gate, s: &Scene, p: &Program, comp: &Compiled, shapes: &[usize], out: &mut Vec<Check>, report: &mut Value) -> Result<(), String> {
    let mut push = |what: String, verdict: Verdict| out.push(Check { gate, scene: s.name.clone(), what, verdict, contended: false });
    let b = &s.budget;
    match gate {
        Gate::Pictures => {
            let l = Loaded::new(c.g, p, c.size, None)?;
            for &t in &c.frames {
                l.frame(c.g, t);
                let px = l.pixels(c.g)?;
                let key = format!("{}@{t}", s.name);
                let path = c.root.join(format!("out/meta/refs/{}_t{t}.png", s.name));
                if c.approve {
                    write_png(&path, c.size, &px)?;
                    c.refs.insert(key.clone(), format!("{:x}", Sha256::digest(&px)));
                    if t == c.frames[0] {
                        let still = c.root.join(format!("agent_notes/2026-11-20_meta/phase1/{}.webp", s.name));
                        std::fs::create_dir_all(still.parent().expect("dir")).map_err(|e| e.to_string())?;
                        let st = std::process::Command::new("cwebp").args(["-quiet", "-q", "95"]).arg(&path).arg("-o").arg(&still).status();
                        if !st.is_ok_and(|x| x.success()) {
                            return Err(format!("cwebp failed for {}", still.display()));
                        }
                    }
                }
                let verdict = match (c.refs.get(&key), read_png(&path)) {
                    (None, _) => Verdict::Unknown { reason: "no approved reference hash in gates.json".into() },
                    (Some(_), Err(e)) => Verdict::Unknown { reason: format!("reference missing: {e}") },
                    (Some(h), Ok((size, r))) => {
                        let file_hash = format!("{:x}", Sha256::digest(&r));
                        if size != c.size || &file_hash != h || !rgba_len(c.size, &px) || !rgba_len(c.size, &r) {
                            Verdict::Unknown { reason: "reference file does not match its approved hash".into() }
                        } else {
                            // Perceptual tolerance: at most 0.1% of pixels may differ by more than 2/255 in a channel.
                            let n = px.len() / 4;
                            let (mut over, mut worst) = (0usize, 0u8);
                            for (a, q) in px.as_chunks::<4>().0.iter().zip(r.as_chunks::<4>().0) {
                                let d = (0..3).map(|i| a[i].abs_diff(q[i])).max().unwrap_or(0);
                                worst = worst.max(d);
                                over += usize::from(d > 2);
                            }
                            report[&s.name]["pictures"][format!("{t}")] = json!({"over_2": over, "max_diff": worst});
                            held_if(0.001 - over as f64 / n as f64, None)
                        }
                    }
                };
                push(format!("frame t={t}"), verdict);
            }
        }
        Gate::Twin => {
            let l = Loaded::new(c.g, p, c.size, None)?;
            for (k, kt) in p.kinds.iter().enumerate() {
                let surface = matches!(kt.draw, Draw::Surface { .. });
                let mut rng = Rng(0x5eed + k as u64);
                let mut samples = vec![];
                let mut at = vec![];
                for inst in p.instances.iter().filter(|i| i.kind == k as u32).take(20) {
                    for _ in 0..32 {
                        let x = if surface { [rng.next(), rng.next(), 0.0] } else { rng.in_box(inst.lo, inst.hi) };
                        let d = rng.dir();
                        samples.extend([x[0], x[1], x[2], f32::from_bits(inst.row), d[0], d[1], d[2], 0.0]);
                        at.push((inst.row as usize, x, d));
                    }
                }
                let gpu = l.twin(c.g, k, &samples)?;
                // trace, visible, material, surface point, surface normal
                let mut evidence: [Evidence; 5] = std::array::from_fn(|_| Evidence::default());
                let mut worst = [0.0f64; 5];
                if gpu.len() != 16 * at.len() { return Err("invalid Twin readback length".into()); }
                for (i, (row, x, d)) in at.iter().enumerate() {
                    let cpu = comp.cpu_query(k, *row, *x, *d).map_err(|e| format!("{e:?}"))?;
                    let o = &gpu[16 * i..16 * i + 16];
                    let pairs = [(0, cpu.trace, o[0]), (1, cpu.visible, o[1])]
                        .into_iter()
                        .chain((0..3).map(|j| (2, cpu.material[j], o[4 + j])))
                        .chain((0..3).map(|j| (3, cpu.surf_p[j], o[8 + j])))
                        .chain((0..3).map(|j| (4, cpu.surf_n[j], o[12 + j])));
                    for (q, a, gv) in pairs {
                        let err = twin_error(q, a, gv);
                        if err > worst[q] && q == 2 {
                            report[&s.name]["twin_worst_material"][&kt.name] = json!({"cpu": cpu.material, "gpu": [o[4], o[5], o[6]], "at": x, "dir": d});
                        }
                        worst[q] = worst[q].max(err);
                        evidence[q].add(TWIN_TOL[q] - err);
                    }
                }
                report[&s.name]["twin"][&kt.name] = json!({"samples": at.len(), "max_err": {"trace": worst[0], "visible": worst[1], "material": worst[2], "surface_p": worst[3], "surface_n": worst[4]}});
                for (q, name) in ["trace", "visible", "material", "surface point", "surface normal"].iter().enumerate() {
                    let used = match kt.draw {
                        Draw::Analytic => q <= 2,
                        Draw::Traced => q == 0 || q == 2,
                        Draw::Surface { .. } => q >= 2,
                        Draw::Mesh { .. } => q == 1 || q == 2,
                    };
                    if used {
                        push(format!("{}.{name} (max err {:.2e}, {} samples)", kt.name, worst[q], at.len()), evidence[q].verdict());
                    }
                }
            }
        }
        Gate::Speed => {
            let control = c.g.triangle_control_ms(c.size)?;
            let l = Loaded::new(c.g, p, c.size, None)?;
            let times = (0..30).map(|i| i as f32 / 30.0)
                .chain((0..c.timing_frames).map(|i| c.frames[0] + i as f32 * (1.0 / 60.0)));
            let timing = l.profile_frames(c.g, times, true)?;
            let measured = &timing[30..];
            let commands = measured.iter().map(|f| f.commands)
                .max_by_key(|n| n.abs_diff(b.commands)).ok_or("missing speed frames")?;
            let ms = measured.iter().map(|f| f.gpu_ms).collect::<Vec<_>>();
            let names: Vec<String> = p.passes.iter().map(|x| format!("{x:?}").split('(').next().unwrap_or("").to_string()).collect();
            let pass_p50: Vec<(String, f64)> = names.iter().enumerate().map(|(i, n)| (n.clone(), quantile(&measured.iter().map(|f| f.pass_ms[i]).collect::<Vec<_>>(), 0.5))).collect();
            report[&s.name]["speed_passes_p50_ms"] = json!(pass_p50);
            let (p50, p95) = (quantile(&ms, 0.5), quantile(&ms, 0.95));
            let deciles: Vec<f64> = (0..=10).map(|i| quantile(&ms, f64::from(i) / 10.0)).collect();
            let contended = control > 0.35;
            report[&s.name]["speed"] = json!({"control_p50_ms": control, "contended": contended, "p50_ms": p50, "p95_ms": p95, "commands": commands, "frames": ms.len(), "deciles_ms": deciles, "ms": ms,
                "frame_stamps": measured, "shader_sha256": crate::profile::shader_hashes(p)?});
            push(format!("{commands} GPU commands == {}", b.commands), held_if(-f64::from(commands.abs_diff(b.commands)), None));
            out.push(Check {
                gate, scene: s.name.clone(), contended,
                what: format!("p95 {p95:.3} ms <= {} ms (control p50 {control:.3} ms)", b.p95_ms),
                verdict: held_if(b.p95_ms - p95, None),
            });
        }
        Gate::Load => {
            // Each pair starts with empty derivations and a fresh driver compilation key.
            let nonce = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.as_nanos()) as u32) & 0x7fff_ffff;
            let cache = tempfile::tempdir().map_err(|e| e.to_string())?;
            let mut times = vec![];
            for _ in 0..2 {
                let t = Instant::now();
                let src = crate::sources(&c.root.join(&s.file))?;
                let set: Vec<_> = s.set.iter().map(|(k, &v)| (k.clone(), v)).collect();
                let (p2, r, _) = loft_passes::compile_cached(&src, &set, cache.path()).map_err(|d| crate::render(&d, &src))?;
                let front = t.elapsed().as_secs_f64() * 1e3;
                let device = Instant::now();
                let l = Loaded::new(c.g, &p2, c.size, Some(nonce))?;
                // Loaded::new waits for device compilation and resource creation.
                let hits: usize = r.kinds.iter().map(|k| k.cache_hits).sum();
                times.push((front, device.elapsed().as_secs_f64() * 1e3, l.stats.clone(), hits));
            }
            let (cold, warm) = ((times[0].0 + times[0].1) / 1e3, (times[1].0 + times[1].1) / 1e3);
            let st = &times[0].2;
            let pipelines = st.render_pipelines + st.compute_pipelines;
            report[&s.name]["load"] = json!({
                "cold_s": cold, "warm_s": warm, "front_ms": [times[0].0, times[1].0],
                "device_ms": [times[0].1, times[1].1], "derivation_hits": [times[0].3, times[1].3],
                "pipelines_ms": [times[0].2.pipelines_ms, times[1].2.pipelines_ms], "each_cold_ms": times[0].2.pipeline_ms,
                "modules": st.modules, "pipelines": pipelines,
            });
            push(format!("{} modules == {}", st.modules, (b.modules as f64)), held_if(-(st.modules as f64 - (b.modules as f64)).abs(), None));
            push(format!("{pipelines} pipelines == {}", (b.pipelines as f64)), held_if(-(pipelines as f64 - (b.pipelines as f64)).abs(), None));
            push(format!("cold {cold:.3} s <= {} s", b.cold_s), held_if(b.cold_s - cold, None));
            push(format!("warm {warm:.3} s <= {} s", b.warm_s), held_if(b.warm_s - warm, None));
            let texts: Vec<String> = p.modules.iter().map(|m| m.wgsl.clone()).collect();
            // Bound derivations: one per distinct local shape. A posed kind whose instances repeat shapes keeps the
            // count fixed; a kind with world-space rows (capsules) or per-instance sizes (blobs) derives per row.
            let n_shapes: usize = shapes.iter().sum();
            report[&s.name]["load"]["shapes"] = json!(shapes);
            push(format!("{n_shapes} bound derivations == {}", (b.shapes as f64)), held_if(-(n_shapes as f64 - (b.shapes as f64)).abs(), None));
            report[&s.name]["load"]["cache_bytes"] = json!(times[0].2.cache_bytes);
            match c.modules.iter().find(|(f, _, _)| *f == s.file) {
                Some((_, other, t)) => {
                    push(format!("modules byte-identical to {other}"), module_parity(t,&texts));
                }
                None => c.modules.push((s.file.clone(), s.name.clone(), texts)),
            }
        }
        Gate::Truth => {
            for &t in &c.frames {
                for (name, value) in authored_checks(c.g, p, t)? {
                    report[&s.name]["authored_checks"][format!("{t}")][&name] = json!(value.to_string());
                    push(format!("authored {name} at t={t}"), value);
                }
            }
            let eye = p.camera.eye(c.frames[0]);
            for (k, kt) in p.kinds.iter().enumerate() {
                let mut rng = Rng(0x7a17 + k as u64);
                let (mut lip, mut lip_worst) = (Evidence::default(), 0.0f64);
                let (mut outside, mut agree, mut hits) = (Evidence::default(), Evidence::default(), 0);
                let cq = |row: usize, x: [f32; 3], d: [f32; 3]| comp.cpu_query(k, row, x, d).map_err(|e| format!("{e:?}"));
                if let Draw::Surface { .. } = kt.draw {
                    // The derived bound holds sampled surface points; the AD normal agrees with independent central
                    // differences of the CPU surface (h = 1e-3 in u and v) wherever the chart is regular.
                    let (mut inside, mut normal, mut worst_angle, mut regular) = (Evidence::default(), Evidence::default(), 0.0f64, 0);
                    let h = 1e-3f32;
                    for inst in p.instances.iter().filter(|i| i.kind == k as u32).take(4) {
                        let row = inst.row as usize;
                        for _ in 0..256 {
                            let (u, v) = (h + (1.0 - 2.0 * h) * rng.next(), h + (1.0 - 2.0 * h) * rng.next());
                            let q = cq(row, [u, v, 0.0], [0.0, 0.0, 1.0])?;
                            for i in 0..3 {
                                inside.add(f64::from(q.surf_p[i] - inst.lo[i]));
                                inside.add(f64::from(inst.hi[i] - q.surf_p[i]));
                            }
                            let at = |u: f32, v: f32| cq(row, [u, v, 0.0], [0.0, 0.0, 1.0]).map(|x| x.surf_p);
                            let (pu0, pu1, pv0, pv1) = (at(u - h, v)?, at(u + h, v)?, at(u, v - h)?, at(u, v + h)?);
                            let du = [0, 1, 2].map(|i| f64::from(pu1[i] - pu0[i]));
                            let dv = [0, 1, 2].map(|i| f64::from(pv1[i] - pv0[i]));
                            let n = [du[1] * dv[2] - du[2] * dv[1], du[2] * dv[0] - du[0] * dv[2], du[0] * dv[1] - du[1] * dv[0]];
                            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                            let scale = (du.iter().map(|x| x * x).sum::<f64>() * dv.iter().map(|x| x * x).sum::<f64>()).sqrt();
                            // A near-degenerate chart (the apex, a pole) has no stable difference normal: skip it.
                            if !len.is_finite() || !scale.is_finite() || !q.surf_n.iter().all(|x| x.is_finite()) {
                                normal.add(f64::NAN);
                                continue;
                            }
                            if len < 0.05 * scale || scale < 1e-12 {
                                continue;
                            }
                            regular += 1;
                            let cosang = (0..3).map(|i| n[i] / len * f64::from(q.surf_n[i])).sum::<f64>();
                            let angle = cosang.clamp(-1.0, 1.0).acos();
                            normal.add(1e-2 - angle);
                            worst_angle = worst_angle.max(angle);
                        }
                    }
                    report[&s.name]["truth"][&kt.name] = json!({"inside_min": inside.min, "normal_angle_max": worst_angle, "regular_samples": regular});
                    push(format!("{}: sampled surface points inside every derived bound", kt.name), inside.verdict());
                    push(format!("{}: AD normal vs differences, worst {worst_angle:.2e} rad <= 1e-2 ({regular} samples)", kt.name), normal.verdict());
                    continue;
                }
                for inst in p.instances.iter().filter(|i| i.kind == k as u32).take(20) {
                    let (row, l) = (inst.row as usize, f64::from(kt.lipschitz[inst.row as usize]));
                    // Derived L: |f(a) - f(b)| <= L |a - b| for pairs inside the row's bound.
                    for _ in 0..128 {
                        let (a, bb) = (rng.in_box(inst.lo, inst.hi), rng.in_box(inst.lo, inst.hi));
                        let dist = (0..3).map(|i| f64::from(a[i] - bb[i]).powi(2)).sum::<f64>().sqrt();
                        if dist < 1e-2 {
                            continue;
                        }
                        let ratio = f64::from((cq(row, a, [0.0, 0.0, 1.0])?.trace - cq(row, bb, [0.0, 0.0, 1.0])?.trace).abs()) / dist;
                        lip_worst = lip_worst.max(ratio);
                        // f32 evaluation noise on both ends, 2e-6 per metre of distance.
                        lip.add(l + 2e-6 / dist - ratio);
                    }
                    // Derived bound: the field is positive just outside the AABB (a shell 0..0.5 m out).
                    for _ in 0..64 {
                        let d = rng.dir();
                        let c0 = [0, 1, 2].map(|i| 0.5 * (inst.lo[i] + inst.hi[i]));
                        let half = [0, 1, 2].map(|i| 0.5 * (inst.hi[i] - inst.lo[i]));
                        // Push the ray from the centre to the box surface, then out by 0..0.5 m.
                        let s_box = (0..3).map(|i| if d[i].abs() > 1e-6 { half[i] / d[i].abs() } else { f32::INFINITY }).fold(f32::INFINITY, f32::min);
                        let x = [0, 1, 2].map(|i| c0[i] + d[i] * (s_box + 1e-3 + 0.5 * rng.next()));
                        outside.add(f64::from(cq(row, x, d)?.trace));
                    }
                    // `visible` agrees with `trace`: where the closed form hits, the field is ~0 there.
                    if kt.draw == loft_passes::program::Draw::Analytic {
                        for _ in 0..64 {
                            let target = rng.in_box(inst.lo, inst.hi);
                            let rd = [0, 1, 2].map(|i| target[i] - eye[i]);
                            let len = (rd[0] * rd[0] + rd[1] * rd[1] + rd[2] * rd[2]).sqrt();
                            let rd = rd.map(|v| v / len);
                            let t = cq(row, eye, rd)?.visible;
                            if !t.is_finite() {
                                agree.add(f64::NAN);
                            } else if t > 0.0 {
                                hits += 1;
                                let x = [0, 1, 2].map(|i| eye[i] + rd[i] * t);
                                agree.add(1e-3 - f64::from(cq(row, x, rd)?.trace.abs()));
                            }
                        }
                    }
                }
                report[&s.name]["truth"][&kt.name] = json!({"derived_L": kt.lipschitz.iter().copied().fold(0.0f32, f32::max), "max_ratio": lip_worst, "outside_min_f": outside.min, "visible_hits": hits});
                push(format!("{}: derived L bounds sampled slopes (max {lip_worst:.4})", kt.name), lip.verdict());
                push(format!("{}: field > 0 outside every derived bound", kt.name), outside.verdict());
                if kt.draw == loft_passes::program::Draw::Analytic {
                    push(format!("{}: visible agrees with trace on {hits} hits (|f| <= 1e-3)", kt.name), agree.verdict());
                }
            }
        }
    }
    Ok(())
}

pub fn run(approve: bool, only: Option<&str>) -> Result<(), Failure> {
    run_selected(approve, only, &GATES)
}

pub(crate) fn speed_gate() -> Result<(), Failure> {
    run_selected(false, None, &[Gate::Speed])
}

fn run_selected(approve: bool, only: Option<&str>, selected: &[Gate]) -> Result<(), Failure> {
    let root = root().map_err(Failure::Usage)?;
    let path = root.join("engines/gates.json");
    let text = std::fs::read_to_string(&path).map_err(|e| Failure::Usage(e.to_string()))?;
    let mut cfg: Manifest = serde_json::from_str(&text).map_err(|e| Failure::Parity(format!("gates.json: {e}")))?;
    let scenes = cfg.select(only, approve)?;
    let size = cfg.size;
    let t = Instant::now();
    let g = loft_cli::native::gpu().map_err(Failure::Device)?;
    let device_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("adapter {}  device {device_ms:.0} ms  {}x{}", g.adapter, size.0, size.1);
    let mut c = Ctx {
        g: &g,
        root: root.clone(),
        size,
        frames: cfg.frames.clone(),
        timing_frames: cfg.timing_frames,
        approve,
        refs: cfg.refs.clone(),
        modules: vec![],
    };
    let mut checks = vec![];
    let mut report = json!({"adapter": g.adapter, "device_ms": device_ms});
    let mut compiled = vec![];
    for s in scenes {
        let file = root.join(&s.file);
        let set: Vec<_> = s.set.iter().map(|(k, &v)| (k.clone(), v)).collect();
        let src = crate::sources(&file).map_err(Failure::Usage)?;
        let (p, r, comp) = loft_passes::compile(&src, &set).map_err(|d| Failure::Compile(d.render(&src)))?;
        report[&s.name]["compile_ms"] = json!(r.ms.iter().map(|(n, t)| json!([n, t])).collect::<Vec<_>>());
        report[&s.name]["rows"] = json!(r.kinds.iter().map(|k| json!([k.name, k.rows, k.shapes, k.l_max])).collect::<Vec<_>>());
        let shapes: Vec<usize> = r.kinds.iter().map(|k| k.shapes).collect();
        if selected.contains(&Gate::Load) {
            gate(&mut c, Gate::Load, s, &p, &comp, &shapes, &mut checks, &mut report).map_err(Failure::Device)?;
        }
        compiled.push((s, p, comp, shapes));
    }
    // Measure all cold/warm loads before the long timestamped frame batches.
    for (s, p, comp, shapes) in compiled {
        for gt in selected.iter().copied().filter(|g| *g != Gate::Load) {
            gate(&mut c, gt, s, &p, &comp, &shapes, &mut checks, &mut report).map_err(Failure::Device)?;
        }
    }
    if approve {
        cfg.refs = c.refs.clone();
        std::fs::write(&path, serde_json::to_string_pretty(&cfg).map_err(|e| Failure::Usage(e.to_string()))? + "\n").map_err(|e| Failure::Usage(e.to_string()))?;
        println!("approved {} references into engines/gates.json and out/meta/refs/", c.refs.len());
    }
    for &gt in selected {
        let mine: Vec<&Check> = checks.iter().filter(|x| x.gate == gt).collect();
        let held = mine.iter().filter(|x| matches!(x.verdict, Verdict::Held { .. })).count();
        let warnings = mine.iter().filter(|x| x.contended).count();
        println!("\n{gt:?}: {held}/{} held, {warnings} contended", mine.len());
        for x in &mine {
            let mark = if x.contended { "CONTENDED" } else if matches!(x.verdict, Verdict::Held { .. }) { "ok  " } else { "FAIL" };
            println!("  {mark} {:<12} {:<64} {}", x.scene, x.what, x.verdict);
        }
    }
    let out = root.join("out/meta/gate.json");
    std::fs::create_dir_all(out.parent().expect("out/meta")).map_err(|e| Failure::Usage(e.to_string()))?;
    std::fs::write(&out, serde_json::to_string_pretty(&report).map_err(|e| Failure::Usage(e.to_string()))?).map_err(|e| Failure::Usage(e.to_string()))?;
    println!("\nreport: {}", out.display());
    result(&checks)
}

fn result(checks:&[Check])->Result<(),Failure>{
    let failed_in = |gate| checks.iter().any(|c| c.gate == gate && !c.contended && !matches!(c.verdict, Verdict::Held { .. }));
    let failed=checks.iter().filter(|c| !c.contended && !matches!(c.verdict, Verdict::Held { .. })).count();
    let message = format!("{failed} checks failed");
    if failed_in(Gate::Truth) { Err(Failure::Truth(message)) }
    else if failed_in(Gate::Pictures) || failed_in(Gate::Twin) { Err(Failure::Parity(message)) }
    else if failed_in(Gate::Load) { Err(Failure::Load(message)) }
    else if failed != 0 { Err(Failure::Budget(message)) }
    else { Ok(()) }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_frames_and_selections_are_strict() {
        let recorded = include_str!("../../../engines/gates.json");
        let manifest: Manifest = serde_json::from_str(recorded).unwrap();
        assert_eq!(manifest.select(None, false).unwrap().len(), 6);
        for selection in ["capsule-1", "", "capsules-1,capsules-1"] {
            assert!(manifest.select(Some(selection), false).is_err());
        }
        let malformed = recorded.replacen("4.0", "\"4.0\"", 1);
        assert!(serde_json::from_str::<Manifest>(&malformed).is_err());
        let deleted = recorded.replacen("    4.0,\n", "", 1);
        let missing: Manifest = serde_json::from_str(&deleted).unwrap();
        assert!(missing.select(Some("capsules-1"), false).is_err());
        assert!(!rgba_len((2, 3), &[0; 23]));
        assert!(rgba_len((2, 3), &[0; 24]));
    }

    #[test]
    fn authored_verdicts_require_a_record_and_finite_evidence() {
        for record in [None, Some([0.0, 0.0]), Some([1.0, f32::NAN]), Some([3.0, 0.0])] {
            assert!(matches!(decode_verdict(record), Verdict::Unknown { .. }));
        }
        assert!(matches!(decode_verdict(Some([2.0, 1.0])), Verdict::Missed { .. }));
        assert!(matches!(decode_verdict(Some([1.0, 1.0])), Verdict::Held { .. }));
    }

    #[test]
    fn parity_uses_absolute_limits_and_requires_finite_samples() {
        let mut e = Evidence::default();
        assert!(matches!(e.verdict(), Verdict::Unknown { .. }));
        e.add(TWIN_TOL[2] - twin_error(2, 100., 100.00006));
        assert!(matches!(e.verdict(), Verdict::Missed { .. }));
        e.add(TWIN_TOL[2] - twin_error(2, f32::NAN, 100.));
        assert!(matches!(e.verdict(), Verdict::Unknown { .. }));
    }
}

fn module_parity(reference:&[String], current:&[String])->Verdict {
    if reference==current {Verdict::Held{margin:0.0}} else {Verdict::Missed{by:1.0}}
}
#[cfg(test)]
#[path="../../../tests/gate_mutants.rs"]
mod gate_mutants;
