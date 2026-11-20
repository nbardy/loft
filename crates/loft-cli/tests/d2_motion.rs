use loft_cli::native::Gpu;
use loft_ir::*;
use loft_passes::fusion::{self, RESULTS, ROWS};
use loft_passes::program::{Dispatch, Program};
use loft_runner::Runner;
use std::{collections::BTreeMap, path::Path, sync::Arc};

const TICKS: usize = 481;
const PASS: PassId = PassId(0);
const EVENTS: TableId = TableId(3);

fn sources(name: &str, scene: &str) -> Sources {
    let lib = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib");
    let mut paths: Vec<_> = std::fs::read_dir(lib)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "loft"))
        .collect();
    paths.sort();
    let mut sources = Sources::default();
    for path in paths {
        sources.add(
            path.display().to_string(),
            std::fs::read_to_string(path).unwrap(),
        );
    }
    sources.add(name, scene);
    sources
}

fn compile(src: &Sources) -> (Program, loft_passes::Compiled) {
    let (program, _, compiled) =
        loft_passes::compile(src, &[]).unwrap_or_else(|d| panic!("{}", d.render(src)));
    (program, compiled)
}

fn words(bytes: &[u8]) -> impl Iterator<Item = u32> {
    bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

fn event_words(event: OutboxEvent) -> [u32; 8] {
    let mut raw = [0; 8];
    raw[..4].copy_from_slice(&[event.tag, event.instance_id, event.site_id, event.tick]);
    raw[4..].copy_from_slice(&event.payload.map(f32::to_bits));
    raw
}

fn initial(bundle: &Bundle) -> BTreeMap<TableId, Vec<u8>> {
    bundle
        .tables
        .iter()
        .map(|t| {
            let mut bytes = t.initial.to_vec();
            bytes.resize((t.capacity * t.stride_bytes) as usize, 0);
            if t.id == EVENTS {
                let capacity = ((bytes.len() - 16) / 32) as u32;
                bytes[4..8].copy_from_slice(&capacity.to_le_bytes());
            }
            (t.id, bytes)
        })
        .collect()
}

struct Trace {
    data: Vec<u8>,
    events: Vec<Vec<[u32; 8]>>,
    drops: u32,
}

fn readback(g: &Gpu, buffer: &wgpu::Buffer) -> Vec<u8> {
    let (send, recv) = std::sync::mpsc::channel();
    buffer.map_async(wgpu::MapMode::Read, .., move |result| {
        send.send(result).unwrap()
    });
    g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    recv.recv().unwrap().unwrap();
    buffer.get_mapped_range(..).unwrap().to_vec()
}

fn gpu_trace(
    g: &Gpu,
    bundle: &Bundle,
    read: &[TableId],
    interval: u32,
    stall: bool,
    ticks: u32,
) -> Trace {
    let fresh = || {
        let mut runner = Runner::attach(
            Arc::new(g.device.clone()),
            Arc::new(g.queue.clone()),
            HostCaps {
                depth_convention: DepthConvention::StandardZeroToOne,
                msaa_samples: 1,
                features: GpuFeatures::empty(),
            },
        );
        let id = runner.load_bundle(bundle).unwrap();
        (runner, id)
    };
    let (mut runner, mut id) = fresh();
    let stride: u64 = read
        .iter()
        .map(|t| runner.export_buffer(id, *t).unwrap().size())
        .sum();
    let dest = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("D2A fixed tick trace"),
        size: u64::from(ticks) * stride,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let has_events = bundle.tables.iter().any(|d| d.id == EVENTS);
    let mut events = vec![vec![]; ticks as usize];
    let mut drops = 0;
    let mut capture = |runner: &mut Runner, id, tick, keep| {
        let mut enc = g.device.create_command_encoder(&Default::default());
        runner.advance_into(id, &mut enc, tick, keep).unwrap();
        let mut offset = u64::from(tick) * stride;
        for t in read {
            let buffer = runner.export_buffer(id, *t).unwrap();
            enc.copy_buffer_to_buffer(&buffer, 0, &dest, offset, Some(buffer.size()));
            offset += buffer.size();
        }
        g.queue.submit([enc.finish()]);
        if has_events {
            g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let (batch, lost) = runner.drain_events(id).unwrap();
            events[tick as usize] = batch
                .into_iter()
                .filter(|e| e.tick == tick)
                .map(event_words)
                .collect();
            drops += lost;
        }
    };
    capture(&mut runner, id, 0, 0);
    let mut start = 0;
    while start + 1 < ticks {
        let end = (start + if stall && start == 120 { 6 } else { interval }).min(ticks - 1);
        // Evaluate every prefix from the boundary and restore before seeking forward.
        for tick in start + 1..end {
            capture(&mut runner, id, tick, 0);
            if start == 0 {
                (runner, id) = fresh();
            } else {
                runner.restore_checkpoint(id, start).unwrap();
            }
        }
        capture(
            &mut runner,
            id,
            end,
            if interval == 1 && !stall { 0 } else { end },
        );
        start = end;
    }
    Trace {
        data: readback(g, &dest),
        events,
        drops,
    }
}

fn cpu_trace(bundle: &Bundle, read: &[TableId]) -> Trace {
    let mut tables = initial(bundle);
    let mut data = vec![];
    let mut events = vec![vec![]; TICKS];
    let mut drops = 0;
    for (tick, writes) in (0u32..).zip(&mut events) {
        Runner::step_bundle_cpu(
            bundle,
            PASS,
            &mut tables,
            InstanceId(0),
            0..1,
            fusion::advance(tick.saturating_sub(1), tick, 0).unwrap(),
        )
        .unwrap();
        for table in read {
            data.extend_from_slice(&tables[table]);
        }
        if let Some(bytes) = tables.get_mut(&EVENTS) {
            let count = words(bytes).next().unwrap() as usize;
            *writes = bytes[16..]
                .chunks_exact(32)
                .take(count)
                .map(|event| words(event).collect::<Vec<_>>().try_into().unwrap())
                .collect();
            drops += words(bytes).nth(2).unwrap();
            bytes[..4].fill(0);
            bytes[8..].fill(0);
        }
    }
    Trace {
        data,
        events,
        drops,
    }
}

fn score(label: &str, a: &Trace, b: &Trace, lengths: &[(TableId, usize)]) -> bool {
    let stride: usize = lengths.iter().map(|(_, bytes)| bytes).sum();
    assert_eq!(a.data.len(), TICKS * stride);
    assert_eq!(b.data.len(), TICKS * stride);
    let mut offset = 0;
    let mut ok = true;
    for &(id, len) in lengths {
        let mut differing = 0;
        for (tick, (a, b)) in a
            .data
            .chunks_exact(stride)
            .zip(b.data.chunks_exact(stride))
            .enumerate()
        {
            let pair = words(&a[offset..offset + len]).zip(words(&b[offset..offset + len]));
            if let Some((i, (x, y))) = pair.enumerate().find(|(_, (x, y))| x != y) {
                println!("{label} table {} t{tick} word {i}: {x:08x}/{y:08x}", id.0);
                differing += 1;
            }
        }
        println!("{label} table {}: {differing}/{TICKS}", id.0);
        ok &= differing == 0;
        offset += len;
    }
    let mut differing = 0;
    for (tick, (a, b)) in a.events.iter().zip(&b.events).enumerate() {
        if a != b {
            println!("{label} outbox t{tick}: {a:08x?}/{b:08x?}");
            differing += 1;
        }
    }
    println!(
        "{label} outbox: {differing}/{TICKS}, drops {}/{}",
        a.drops, b.drops
    );
    ok && differing == 0 && a.drops == 0 && b.drops == 0
}

fn field(step: &Dispatch, row: &[u8], key: &str) -> f32 {
    let offset = step.fields.iter().find(|(name, _)| name == key).unwrap().1;
    f32::from_bits(words(row).nth(offset as usize).unwrap())
}

const CRAB: &str = include_str!("../../../scenes/crab.loft");
const HERON: &str = include_str!("../../../scenes/heron.loft");
const BOSS: &str = include_str!("../../../scenes/boss.loft");

fn scene(name: &str, source: &str) {
    let src = sources(name, source);
    let (program, _) = compile(&src);
    assert_eq!(program.dispatches.len(), 1);
    let step = &program.dispatches[0];
    let bundle = fusion::motion_bundle(&program, step, 3);
    let lengths: Vec<_> = bundle
        .tables
        .iter()
        .filter(|t| [ROWS, RESULTS, TableId(4)].contains(&t.id))
        .map(|t| (t.id, (t.capacity * t.stride_bytes) as usize))
        .collect();
    let read: Vec<_> = lengths.iter().map(|(id, _)| *id).collect();
    for table in &bundle.tables {
        println!(
            "{name} {}::{} table {} stride {} count {}",
            program.modules[step.module].name,
            step.entry,
            table.id.0,
            table.stride_bytes,
            table.capacity
        );
    }
    println!("{name} fields {:?}, checks {:?}", step.fields, step.checks);
    let g = loft_cli::native::gpu().unwrap();
    println!("{}", g.device_signature);
    let cpu = cpu_trace(&bundle, &read);
    let gpu = gpu_trace(&g, &bundle, &read, 1, false, TICKS as u32);
    let mut ok = score("CPU/Metal 120 Hz", &cpu, &gpu, &lengths);
    for (label, interval, stall) in [
        ("60 Hz plus seek", 2, false),
        ("30 Hz plus seek", 4, false),
        ("30 Hz six-tick stall/replay", 4, true),
    ] {
        let sample = gpu_trace(&g, &bundle, &read, interval, stall, TICKS as u32);
        ok &= score(label, &gpu, &sample, &lengths);
    }
    let stride: usize = lengths.iter().map(|(_, len)| len).sum();
    let row_len = lengths.iter().find(|(id, _)| *id == ROWS).unwrap().1;
    let row = |t| &gpu.data[t * stride..t * stride + row_len];
    let val = |t, key| field(step, row(t), key);
    let verdict = |t, j| {
        let at = t * stride + row_len + j * 32;
        let rec = &gpu.data[at..at + 32];
        (
            words(rec).nth(4).unwrap(),
            f32::from_bits(words(rec).nth(6).unwrap()),
        )
    };
    match name {
        "crab" => {
            let lift = (1..TICKS).find(|&t| val(t, "phase") > 0.).unwrap();
            let drift = (0..TICKS)
                .map(|t| {
                    let (tag, margin) = verdict(t, 0);
                    if tag == 1 { 0.00005 - margin } else { margin }
                })
                .fold(0., f32::max);
            println!(
                "Crab lift t{lift}, {:.9} s; max drift {:.9} mm; x(t2) {:08x}",
                lift as f64 / 120.,
                drift * 1000.,
                val(2, "x").to_bits()
            );
            ok &= lift <= 12 && drift <= 0.00005;
        }
        "heron" => {
            let low = (0..TICKS)
                .map(|t| val(t, "hip_y"))
                .fold(f32::INFINITY, f32::min);
            let max_pitch = (0..TICKS)
                .map(|t| val(t, "lean").atan().abs().to_degrees())
                .fold(0., f32::max);
            let neck = (0..TICKS)
                .map(|t| {
                    let (tag, margin) = verdict(t, 1);
                    if tag == 1 { 0.0002 - margin } else { margin }
                })
                .fold(0., f32::max);
            let flips = (0..TICKS).filter(|&t| verdict(t, 3).0 != 1).count();
            let knot = val(72, "hip_y");
            println!(
                "Heron minimum {low:.9} m; knot {knot:.9}/{:08x}; pitch {max_pitch:.9} deg; neck {:.9} mm; rear gaze failures {flips}",
                knot.to_bits(),
                neck * 1000.
            );
            ok &= knot.to_bits() == 0x3f170a3d
                && (low - 0.59).abs() < 0.00005
                && max_pitch <= 12.
                && neck <= 0.0002
                && flips == 0;
        }
        "boss" => {
            let high = (0..TICKS).map(|t| val(t, "windup")).fold(0., f32::max);
            let hits: Vec<_> = (1..TICKS)
                .filter(|&t| val(t, "hits") != val(t - 1, "hits"))
                .map(|t| (t, val(t, "slipping")))
                .collect();
            let events: Vec<_> = gpu.events.iter().flatten().copied().collect();
            println!(
                "Boss max windup {high:.9} m; t8 {:08x}; hits/slip {hits:?}; exact outbox {events:08x?}",
                val(8, "windup").to_bits()
            );
            let expected = [
                [
                    fusion::event_tag("ClawHit"),
                    0,
                    1,
                    60,
                    (-1800f32).to_bits(),
                    (-900f32).to_bits(),
                    0,
                    420f32.to_bits(),
                ],
                [
                    fusion::event_tag("FootSlip"),
                    0,
                    0,
                    60,
                    0,
                    0.06f32.to_bits(),
                    0,
                    0.35f32.to_bits(),
                ],
            ];
            ok &= (high - 0.69).abs() < 0.00005
                && hits == [(60, 1.0), (75, 1.0)]
                && events == expected;
        }
        _ => unreachable!("the three authored scenes"),
    }
    assert!(ok);
}

#[test]
fn crab() {
    scene("crab", CRAB);
}

#[test]
fn heron() {
    scene("heron", HERON);
}

#[test]
fn boss() {
    scene("boss", BOSS);
}

#[test]
fn prior_non_knot_reaches_heron_in_both_evaluators() {
    use loft_passes::eval::{Eval, Val};
    use loft_passes::parse::ast::{ExprKind, Stmt};
    let src = sources("heron", HERON);
    let (mut p, compiled) = compile(&src);
    let header = p.priors["HeronHop"] as usize;
    assert_eq!(p.luts[header + 1].to_bits(), 97);
    let table = p.luts[header + 2].to_bits() as usize;
    let step = &p.dispatches[0];
    let prog = compiled.prog();
    let law = &prog.laws["heron_step"].f;
    let source_hip = |p: &Program| {
        let mut eval = Eval::<f32>::new(&prog, &compiled.types, compiled.tables.clone(), &[]);
        eval.bind_priors(&p.priors, &p.luts);
        eval.time = 1.0 / 120.0;
        let row = prog.kind_index("HuntingHeron");
        eval.bind("h", Val::Row(row, 0));
        for stmt in &law.body.stmts {
            match stmt {
                Stmt::Let { name, value, .. } => {
                    let v = eval.expr(value).unwrap();
                    eval.bind(&name.name, v);
                }
                Stmt::Assign { target, value, .. }
                    if matches!(&target.kind,
                    ExprKind::Member(_, member) if member.name == "hip_y") =>
                {
                    let Val::S(y) = eval.expr(value).unwrap() else {
                        panic!("hip scalar")
                    };
                    return y.to_bits();
                }
                _ => {}
            }
        }
        panic!("heron hip write");
    };
    let g = loft_cli::native::gpu().unwrap();
    let gpu_hip = |p: &Program| {
        let b = fusion::motion_bundle(p, step, 0);
        let trace = gpu_trace(&g, &b, &[ROWS], 1, false, 2);
        field(step, &trace.data[trace.data.len() / 2..], "hip_y").to_bits()
    };
    let original = (source_hip(&p), gpu_hip(&p));
    p.luts[table + 12 + 1] += 0.02;
    let changed = (source_hip(&p), gpu_hip(&p));
    p.luts[header + 3] = f32::from_bits(3);
    let failed = (source_hip(&p), gpu_hip(&p));
    println!("Heron t1 source/Metal {original:08x?} -> {changed:08x?}; failed plan {failed:08x?}");
    assert_eq!(original.0, original.1);
    assert_eq!(changed.0, changed.1);
    assert_ne!(original.0, changed.0);
    assert_eq!(failed, (0.85f32.to_bits(), 0.85f32.to_bits()));
}

#[test]
fn refuses_raw_unverified_effects() {
    const RAW: &str = r"
fn raw(x: f) -> f = wgsl det (x: f) -> f { RAW }
state Probe { x: f = 1.0 }
solver Run over Probe writes Probe.{x} { x = raw(x) }
schedule Main { run_solver(Run) }
";
    for body in [
        "loop {} return x;",
        "var v: f16 = f16(x); return f32(v);",
        "return atomicAdd(&loft_tick_time, x);",
        "loft_tick_time = x; return x;",
        "return pow(x, x);",
    ] {
        let text =
            CRAB.split_once("schedule Main").unwrap().0.to_owned() + &RAW.replace("RAW", body);
        let src = sources("raw-refusal", &text);
        let Err(err) = loft_passes::compile(&src, &[]) else {
            panic!("accepted {body}")
        };
        println!("{}", err.render(&src));
        assert_eq!(err.code, Code::E504);
        assert_eq!(err.span.file as usize, src.files.len() - 1);
    }
}

#[test]
fn crab_toe_mutant_and_reachable_constraint() {
    let g = loft_cli::native::gpu().unwrap();
    let verdict = |source: &str| {
        let (p, _) = compile(&sources("crab-probe", source));
        let b = fusion::motion_bundle(&p, &p.dispatches[0], 0);
        let trace = gpu_trace(&g, &b, &[RESULTS], 1, false, 1);
        let (tag, value) = (
            words(&trace.data).nth(4).unwrap(),
            words(&trace.data).nth(6).unwrap(),
        );
        (tag, value)
    };
    let base = verdict(CRAB);
    let toe = verdict(&CRAB.replace("toe_bias: f = 0.0", "toe_bias: f = 0.02"));
    let constraint = verdict(&CRAB.replace("const CRAB_LONG = 0.68m", "const CRAB_LONG = 0.21m"));
    println!(
        "Crab planted tag/margin t0 base {base:08x?}, toe {toe:08x?}, reach constraint {constraint:08x?}"
    );
    assert_eq!(base.0, 1);
    assert_eq!(toe.0, 2);
    assert!((f32::from_bits(toe.1) - 0.02).abs() < 0.00005);
    assert_eq!(constraint.0, 2);
    assert_ne!(constraint.1, toe.1);
}

#[test]
fn raw_binary32_words() {
    let fixture = r"
fn ops(a: f, b: f, c: f) -> v4 = wgsl det (a: f, b: f, c: f) -> v4 {
    return vec4f(a * b + c, fma(a, b, c), a / b, sqrt(a));
}
state Operands { a: f, b: f, c: f, result: v4 }
solver Arithmetic over Operands writes Operands.{result} { result = ops(a, b, c) }
schedule Main { run_solver(Arithmetic) }
";
    let text = CRAB.split_once("schedule Main").unwrap().0.to_owned() + fixture;
    let (p, _) = compile(&sources("binary32.loft", &text));
    let mut b = fusion::motion_bundle(&p, &p.dispatches[0], 0);
    let cases: [[u32; 3]; 8] = [
        [0x3f800001, 0x3f7ffffe, 0xbf800000], // product rounds to one, exact FMA stays negative
        [1, 0x40000000, 0],                   // subnormal arithmetic
        [0x00800000, 0x40400000, 0],
        [0x7f7fffff, 0x40000000, 0xff800000], // finite product, infinite addend
        [0x80000000, 0x40000000, 0x80000000], // signed zero
        [0x7f800000, 0, 0],
        [0x7fc00123, 0x3f800000, 0],
        [0x3f9e0419, 0x3f6b8a92, 0xbf000001],
    ];
    let table = b.tables.iter_mut().find(|t| t.id == ROWS).unwrap();
    let stride = table.stride_bytes as usize;
    assert_eq!(stride, 28);
    table.capacity = cases.len() as u32;
    table.initial = cases
        .iter()
        .flat_map(|v| v.iter().copied().chain([0; 4]))
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>()
        .into();
    let mut cpu = initial(&b);
    Runner::step_bundle_cpu(
        &b,
        PASS,
        &mut cpu,
        InstanceId(0),
        0..cases.len() as u32,
        fusion::advance(0, 1, 0).unwrap(),
    )
    .unwrap();
    let g = loft_cli::native::gpu().unwrap();
    let trace = gpu_trace(&g, &b, &[ROWS], 1, false, 2);
    let bytes = &trace.data[cases.len() * stride..];
    for (i, [a, b, c]) in cases
        .map(|row| row.map(f32::from_bits))
        .into_iter()
        .enumerate()
    {
        let expected = [a * b + c, a.mul_add(b, c), a / b, a.sqrt()]
            .map(|x| if x.is_nan() { 0x7fc00000 } else { x.to_bits() });
        let cpu: Vec<_> = words(&cpu[&ROWS][i * stride..][..stride]).skip(3).collect();
        let gpu: Vec<_> = words(&bytes[i * stride..][..stride]).skip(3).collect();
        println!(
            "binary32 input {:08x?} expected {expected:08x?}, CPU {cpu:08x?}, Metal {gpu:08x?}",
            cases[i]
        );
        assert_eq!(cpu, expected);
        assert_eq!(gpu, expected);
    }
}
