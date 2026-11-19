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

use loft_ir::Verdict;
use loft_ir::program::{Draw, Program};
use loft_passes::Compiled;
use loft_runner::{Gpu, Loaded};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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

struct Scene {
    name: String,
    file: PathBuf,
    set: Vec<(String, f64)>,
    budget: Value,
}

struct Check {
    gate: Gate,
    scene: String,
    what: String,
    verdict: Verdict,
}

/// Tolerances of the twin (draft §4.4): |Δ| <= 1e-5 abs or 8 ULP, whichever is larger.
fn twin_tol(v: f32) -> f64 {
    1e-5f64.max(8.0 * f64::from(v.abs()) * f64::from(f32::EPSILON))
}

fn held_if(slack: f64, unknown: Option<String>) -> Verdict {
    match unknown {
        Some(reason) => Verdict::Unknown { reason },
        None if slack >= 0.0 => Verdict::Held { margin: slack },
        None => Verdict::Missed { by: -slack },
    }
}

fn root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    cwd.ancestors()
        .find(|d| d.join("engines/gates.json").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| "run from inside the repository: no engines/gates.json above the current directory".into())
}

fn compile(file: &Path, set: &[(String, f64)]) -> Result<(Program, loft_passes::Report, Compiled, f64), String> {
    let src = crate::sources(file)?;
    let t = Instant::now();
    let (p, r, c) = loft_passes::compile(&src, set).map_err(|d| crate::render(&d, &src))?;
    Ok((p, r, c, t.elapsed().as_secs_f64() * 1e3))
}

fn write_png(path: &Path, size: (u32, u32), rgba: &[u8]) -> Result<(), String> {
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

pub fn still(scene: &Path, set: &[(String, f64)], t: f32, out: &Path) -> Result<(), String> {
    let g = loft_runner::gpu()?;
    let (p, _, _, _) = compile(scene, set)?;
    let size = (1280, 800);
    let l = Loaded::new(&g, &p, size, None)?;
    l.frame(&g, t);
    let px = l.pixels(&g)?;
    write_png(out, size, &px)?;
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

fn quantile(xs: &[f64], q: f64) -> f64 {
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
    refs: serde_json::Map<String, Value>,
    /// Module texts per scene file, to check identity across instance counts.
    modules: Vec<(PathBuf, String, Vec<String>)>,
}

/// One gate on one scene: its verdicts, plus numbers for the report.
#[allow(clippy::too_many_arguments)]
fn gate(c: &mut Ctx, gate: Gate, s: &Scene, p: &Program, comp: &Compiled, shapes: &[usize], out: &mut Vec<Check>, report: &mut Value) -> Result<(), String> {
    let mut push = |what: String, verdict: Verdict| out.push(Check { gate, scene: s.name.clone(), what, verdict });
    let b = |k: &str| s.budget.get(k).and_then(Value::as_f64).ok_or_else(|| format!("{}: budget has no `{k}`", s.name));
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
                    c.refs.insert(key.clone(), json!(format!("{:x}", Sha256::digest(&px))));
                    if t == c.frames[0] {
                        let still = c.root.join(format!("agent_notes/2026-11-20_meta/phase1/{}.webp", s.name));
                        std::fs::create_dir_all(still.parent().expect("dir")).map_err(|e| e.to_string())?;
                        let st = std::process::Command::new("cwebp").args(["-quiet", "-q", "95"]).arg(&path).arg("-o").arg(&still).status();
                        if !st.is_ok_and(|x| x.success()) {
                            return Err(format!("cwebp failed for {}", still.display()));
                        }
                    }
                }
                let verdict = match (c.refs.get(&key).and_then(Value::as_str), read_png(&path)) {
                    (None, _) => Verdict::Unknown { reason: "no approved reference hash in gates.json".into() },
                    (Some(_), Err(e)) => Verdict::Unknown { reason: format!("reference missing: {e}") },
                    (Some(h), Ok((size, r))) => {
                        let file_hash = format!("{:x}", Sha256::digest(&r));
                        if size != c.size || file_hash != h {
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
                let (mut slack, mut worst) = ([f64::INFINITY; 5], [0.0f64; 5]);
                for (i, (row, x, d)) in at.iter().enumerate() {
                    let cpu = comp.cpu_query(k, *row, *x, *d).map_err(|e| format!("{e:?}"))?;
                    let o = &gpu[16 * i..16 * i + 16];
                    let pairs = [(0, cpu.trace, o[0]), (1, cpu.visible, o[1])]
                        .into_iter()
                        .chain((0..3).map(|j| (2, cpu.material[j], o[4 + j])))
                        .chain((0..3).map(|j| (3, cpu.surf_p[j], o[8 + j])))
                        .chain((0..3).map(|j| (4, cpu.surf_n[j], o[12 + j])));
                    for (q, a, gv) in pairs {
                        let err = f64::from((a - gv).abs());
                        // `visible` misses are any negative value: compare hit/miss, then t on hits.
                        let err = if q == 1 && (a < 0.0 || gv < 0.0) { if (a < 0.0) == (gv < 0.0) { 0.0 } else { f64::INFINITY } } else { err };
                        worst[q] = worst[q].max(err);
                        slack[q] = slack[q].min(twin_tol(a) - err);
                    }
                }
                report[&s.name]["twin"][&kt.name] = json!({"samples": at.len(), "max_err": {"trace": worst[0], "visible": worst[1], "material": worst[2], "surface_p": worst[3], "surface_n": worst[4]}});
                for (q, name) in ["trace", "visible", "material", "surface point", "surface normal"].iter().enumerate() {
                    let used = match kt.draw {
                        Draw::Analytic => q <= 2,
                        Draw::Traced => q == 0 || q == 2,
                        Draw::Surface { .. } => q >= 2,
                    };
                    if used {
                        push(format!("{}.{name} (max err {:.2e}, {} samples)", kt.name, worst[q], at.len()), held_if(slack[q], None));
                    }
                }
            }
        }
        Gate::Speed => {
            let l = Loaded::new(c.g, p, c.size, None)?;
            let mut commands = 0;
            for i in 0..30 {
                commands = l.frame(c.g, i as f32 / 30.0);
                l.gpu_ms(c.g)?;
            }
            let mut ms = vec![];
            for i in 0..c.timing_frames {
                // The orbit moves so every frame is a different view of the same scene.
                l.frame(c.g, c.frames[0] + i as f32 * (1.0 / 60.0));
                ms.push(l.gpu_ms(c.g)?.ok_or("no timestamp queries on this adapter")?);
            }
            let (p50, p95) = (quantile(&ms, 0.5), quantile(&ms, 0.95));
            let deciles: Vec<f64> = (0..=10).map(|i| quantile(&ms, f64::from(i) / 10.0)).collect();
            report[&s.name]["speed"] = json!({"p50_ms": p50, "p95_ms": p95, "commands": commands, "frames": ms.len(), "deciles_ms": deciles, "ms": ms});
            push(format!("p95 {p95:.3} ms (p50 {p50:.3}) <= {} ms", b("p95_ms")?), held_if(b("p95_ms")? - p95, None));
            push(format!("{commands} GPU commands == {}", b("commands")?), held_if(-(f64::from(commands) - b("commands")?).abs(), None));
        }
        Gate::Load => {
            // Cold: a nonce the driver has never seen; warm: the same nonce again in this process.
            let nonce = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.as_nanos()) as u32) & 0x7fff_ffff;
            let mut times = vec![];
            for _ in 0..2 {
                let (p2, _, _, front) = compile(&s.file, &s.set)?;
                let t = Instant::now();
                let l = Loaded::new(c.g, &p2, c.size, Some(nonce))?;
                l.frame(c.g, 0.0);
                l.gpu_ms(c.g)?;
                times.push((front, t.elapsed().as_secs_f64() * 1e3, l.stats.clone()));
            }
            let (cold, warm) = ((times[0].0 + times[0].1) / 1e3, (times[1].0 + times[1].1) / 1e3);
            let st = &times[0].2;
            let pipelines = st.render_pipelines + st.compute_pipelines;
            report[&s.name]["load"] = json!({
                "cold_s": cold, "warm_s": warm, "front_ms": [times[0].0, times[1].0],
                "pipelines_ms": [times[0].2.pipelines_ms, times[1].2.pipelines_ms],
                "modules": st.modules, "pipelines": pipelines,
            });
            push(format!("{} modules == {}", st.modules, b("modules")?), held_if(-(st.modules as f64 - b("modules")?).abs(), None));
            push(format!("{pipelines} pipelines == {}", b("pipelines")?), held_if(-(pipelines as f64 - b("pipelines")?).abs(), None));
            push(format!("cold {cold:.3} s <= {} s", b("cold_s")?), held_if(b("cold_s")? - cold, None));
            push(format!("warm {warm:.3} s <= {} s", b("warm_s")?), held_if(b("warm_s")? - warm, None));
            let mut texts: Vec<String> = p.modules.iter().map(|m| m.wgsl.clone()).collect();
            // The derivation count (distinct shapes) rides along: it must not grow with instances either.
            texts.push(format!("shapes {:?}", shapes));
            report[&s.name]["load"]["shapes"] = json!(shapes);
            report[&s.name]["load"]["cache_bytes"] = json!(times[0].2.cache_bytes);
            match c.modules.iter().find(|(f, _, _)| *f == s.file) {
                Some((_, other, t)) => {
                    let same = *t == texts;
                    push(format!("modules and shape count identical to {other}"), if same { Verdict::Held { margin: 0.0 } } else { Verdict::Missed { by: 1.0 } });
                }
                None => c.modules.push((s.file.clone(), s.name.clone(), texts)),
            }
        }
        Gate::Truth => {
            let eye = p.camera.eye(c.frames[0]);
            for (k, kt) in p.kinds.iter().enumerate() {
                let mut rng = Rng(0x7a17 + k as u64);
                let (mut lip_slack, mut lip_worst) = (f64::INFINITY, 0.0f64);
                let (mut out_slack, mut agree_slack, mut hits) = (f64::INFINITY, f64::INFINITY, 0);
                let cq = |row: usize, x: [f32; 3], d: [f32; 3]| comp.cpu_query(k, row, x, d).map_err(|e| format!("{e:?}"));
                if let Draw::Surface { .. } = kt.draw {
                    // The derived bound holds sampled surface points; the AD normal agrees with independent central
                    // differences of the CPU surface (h = 1e-3 in u and v) wherever the chart is regular.
                    let (mut in_slack, mut worst_angle, mut regular) = (f64::INFINITY, 0.0f64, 0);
                    let h = 1e-3f32;
                    for inst in p.instances.iter().filter(|i| i.kind == k as u32).take(4) {
                        let row = inst.row as usize;
                        for _ in 0..256 {
                            let (u, v) = (h + (1.0 - 2.0 * h) * rng.next(), h + (1.0 - 2.0 * h) * rng.next());
                            let q = cq(row, [u, v, 0.0], [0.0, 0.0, 1.0])?;
                            for i in 0..3 {
                                in_slack = in_slack.min(f64::from((q.surf_p[i] - inst.lo[i]).min(inst.hi[i] - q.surf_p[i])));
                            }
                            let at = |u: f32, v: f32| cq(row, [u, v, 0.0], [0.0, 0.0, 1.0]).map(|x| x.surf_p);
                            let (pu0, pu1, pv0, pv1) = (at(u - h, v)?, at(u + h, v)?, at(u, v - h)?, at(u, v + h)?);
                            let du = [0, 1, 2].map(|i| f64::from(pu1[i] - pu0[i]));
                            let dv = [0, 1, 2].map(|i| f64::from(pv1[i] - pv0[i]));
                            let n = [du[1] * dv[2] - du[2] * dv[1], du[2] * dv[0] - du[0] * dv[2], du[0] * dv[1] - du[1] * dv[0]];
                            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                            let scale = (du.iter().map(|x| x * x).sum::<f64>() * dv.iter().map(|x| x * x).sum::<f64>()).sqrt();
                            // A near-degenerate chart (the apex, a pole) has no stable difference normal: skip it.
                            if len < 0.05 * scale || scale < 1e-12 {
                                continue;
                            }
                            regular += 1;
                            let cosang = (0..3).map(|i| n[i] / len * f64::from(q.surf_n[i])).sum::<f64>();
                            worst_angle = worst_angle.max(cosang.clamp(-1.0, 1.0).acos());
                        }
                    }
                    report[&s.name]["truth"][&kt.name] = json!({"inside_min": in_slack, "normal_angle_max": worst_angle, "regular_samples": regular});
                    push(format!("{}: sampled surface points inside every derived bound", kt.name), held_if(in_slack, None));
                    let unknown = (regular == 0).then(|| "no regular sample".to_string());
                    push(format!("{}: AD normal vs differences, worst {worst_angle:.2e} rad <= 1e-2 ({regular} samples)", kt.name), held_if(1e-2 - worst_angle, unknown));
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
                        lip_slack = lip_slack.min(l + 2e-6 / dist - ratio);
                    }
                    // Derived bound: the field is positive just outside the AABB (a shell 0..0.5 m out).
                    for _ in 0..64 {
                        let d = rng.dir();
                        let c0 = [0, 1, 2].map(|i| 0.5 * (inst.lo[i] + inst.hi[i]));
                        let half = [0, 1, 2].map(|i| 0.5 * (inst.hi[i] - inst.lo[i]));
                        // Push the ray from the centre to the box surface, then out by 0..0.5 m.
                        let s_box = (0..3).map(|i| if d[i].abs() > 1e-6 { half[i] / d[i].abs() } else { f32::INFINITY }).fold(f32::INFINITY, f32::min);
                        let x = [0, 1, 2].map(|i| c0[i] + d[i] * (s_box + 1e-3 + 0.5 * rng.next()));
                        out_slack = out_slack.min(f64::from(cq(row, x, d)?.trace));
                    }
                    // `visible` agrees with `trace`: where the closed form hits, the field is ~0 there.
                    if kt.draw == loft_ir::program::Draw::Analytic {
                        for _ in 0..64 {
                            let target = rng.in_box(inst.lo, inst.hi);
                            let rd = [0, 1, 2].map(|i| target[i] - eye[i]);
                            let len = (rd[0] * rd[0] + rd[1] * rd[1] + rd[2] * rd[2]).sqrt();
                            let rd = rd.map(|v| v / len);
                            let t = cq(row, eye, rd)?.visible;
                            if t > 0.0 {
                                hits += 1;
                                let x = [0, 1, 2].map(|i| eye[i] + rd[i] * t);
                                agree_slack = agree_slack.min(1e-3 - f64::from(cq(row, x, rd)?.trace.abs()));
                            }
                        }
                    }
                }
                report[&s.name]["truth"][&kt.name] = json!({"derived_L": kt.lipschitz.iter().copied().fold(0.0f32, f32::max), "max_ratio": lip_worst, "outside_min_f": out_slack, "visible_hits": hits});
                push(format!("{}: derived L bounds sampled slopes (max {lip_worst:.4})", kt.name), held_if(lip_slack, None));
                push(format!("{}: field > 0 outside every derived bound", kt.name), held_if(out_slack, None));
                if kt.draw == loft_ir::program::Draw::Analytic {
                    let unknown = (hits == 0).then(|| "no sampled ray hit".to_string());
                    push(format!("{}: visible agrees with trace on {hits} hits (|f| <= 1e-3)", kt.name), held_if(agree_slack, unknown));
                }
            }
        }
    }
    Ok(())
}

pub fn run(approve: bool) -> Result<(), String> {
    let root = root()?;
    let path = root.join("engines/gates.json");
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut cfg: Value = serde_json::from_str(&text).map_err(|e| format!("gates.json: {e}"))?;
    let size = cfg["size"].as_array().and_then(|a| Some((a.first()?.as_u64()? as u32, a.get(1)?.as_u64()? as u32))).ok_or("gates.json: size [w, h]")?;
    let frames: Vec<f32> = cfg["frames"].as_array().ok_or("gates.json: frames")?.iter().filter_map(Value::as_f64).map(|x| x as f32).collect();
    let scenes: Vec<Scene> = cfg["scenes"]
        .as_array()
        .ok_or("gates.json: scenes")?
        .iter()
        .map(|s| {
            Some(Scene {
                name: s["name"].as_str()?.into(),
                file: root.join(s["file"].as_str()?),
                set: s["set"].as_object().map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_f64()?))).collect()).unwrap_or_default(),
                budget: s["budget"].clone(),
            })
        })
        .collect::<Option<_>>()
        .ok_or("gates.json: each scene needs name, file, budget")?;
    let t = Instant::now();
    let g = loft_runner::gpu()?;
    let device_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("adapter {}  device {device_ms:.0} ms  {}x{}", g.adapter, size.0, size.1);
    let mut c = Ctx {
        g: &g,
        root: root.clone(),
        size,
        frames,
        timing_frames: cfg["timing_frames"].as_u64().unwrap_or(300) as usize,
        approve,
        refs: cfg["refs"].as_object().cloned().unwrap_or_default(),
        modules: vec![],
    };
    let mut checks = vec![];
    let mut report = json!({"adapter": g.adapter, "device_ms": device_ms});
    for s in &scenes {
        let (p, r, comp, _) = compile(&s.file, &s.set)?;
        report[&s.name]["compile_ms"] = json!(r.ms.iter().map(|(n, t)| json!([n, t])).collect::<Vec<_>>());
        report[&s.name]["rows"] = json!(r.kinds.iter().map(|k| json!([k.name, k.rows, k.shapes, k.l_max])).collect::<Vec<_>>());
        let shapes: Vec<usize> = r.kinds.iter().map(|k| k.shapes).collect();
        for gt in GATES {
            gate(&mut c, gt, s, &p, &comp, &shapes, &mut checks, &mut report)?;
        }
    }
    if approve {
        cfg["refs"] = Value::Object(c.refs.clone());
        std::fs::write(&path, serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())? + "\n").map_err(|e| e.to_string())?;
        println!("approved {} references into engines/gates.json and out/meta/refs/", c.refs.len());
    }
    let mut failed = 0;
    for gt in GATES {
        let mine: Vec<&Check> = checks.iter().filter(|x| x.gate == gt).collect();
        let held = mine.iter().filter(|x| matches!(x.verdict, Verdict::Held { .. })).count();
        println!("\n{gt:?}: {held}/{} held", mine.len());
        for x in &mine {
            let mark = if matches!(x.verdict, Verdict::Held { .. }) { "ok  " } else { "FAIL" };
            println!("  {mark} {:<12} {:<64} {}", x.scene, x.what, x.verdict);
        }
        failed += mine.len() - held;
    }
    let out = root.join("out/meta/gate.json");
    std::fs::create_dir_all(out.parent().expect("out/meta")).map_err(|e| e.to_string())?;
    std::fs::write(&out, serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("\nreport: {}", out.display());
    if failed == 0 { Ok(()) } else { Err(format!("{failed} checks not Held")) }
}
