use crate::{
    budget::{self, Failure},
    gates::{quantile, write_png},
};
use loft_cli::native::{Gpu, Loaded, TraceSession};
use loft_ir::Sources;
use loft_passes::{
    program::{Program, abi},
    tier0::{PreparedEdit, TraceSource},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

const SIZE: (u32, u32) = (1280, 800);
const TIMING_FRAMES: usize = 120;
const WARMUP_FRAMES: usize = 12;
const EDIT_MS: f64 = 2.0;
const FRAME_MS: f64 = 0.5;
const SAME_MS: f64 = 0.10;
const TRIANGLE_MS: f64 = 0.35;

pub fn edited_still(
    base: &Path,
    edit: &Path,
    kind: &str,
    set: &[(String, f64)],
    t: f32,
    out: &Path,
    max_frame: Option<f64>,
) -> Result<(), Failure> {
    let base = crate::sources(base).map_err(Failure::Usage)?;
    let gpu = loft_cli::native::gpu().map_err(Failure::Device)?;
    let mut tier0 = session(&gpu, &base, kind, set)?;
    let mut tier1 = session(&gpu, &base, kind, set)?;
    let clean0 = tier0.clean_pipelines();
    let clean1 = tier1.clean_pipelines();
    tier0.loaded.frame(&gpu, t);
    let before = tier0.loaded.pixels(&gpu).map_err(Failure::Device)?;

    let edited = crate::sources(edit).map_err(Failure::Usage)?;
    let received = Instant::now();
    let receipt = tier0
        .edit(&gpu, &edited, set, received)
        .map_err(Failure::Compile)?;
    let uploaded = Instant::now();
    println!(
        "{:?} generation {}: {}",
        receipt.class,
        receipt.generation,
        tier0.describe()
    );
    println!(
        "dirtied rows {:?}, tape byte ranges {:?}, caster bound changed {}",
        receipt.dirtied_rows, receipt.tape_writes, receipt.caster_changed
    );
    println!(
        "source-to-upload {:.6} ms, parse/check {:.6} ms, bounds/lowering {:.6} ms, queue upload {:.6} ms, foreground pipeline creations {}",
        millis(receipt.elapsed),
        millis(receipt.parse_check),
        millis(receipt.bounds_lowering),
        millis(receipt.upload),
        receipt.foreground_creates
    );

    let score = std::env::var_os("LOFT_D2B_SCORE").is_some();
    if score || max_frame.is_some() {
        budget::timestamps(gpu.timestamps)?;
    }
    // Deliberately leave a prior generation in flight in the promoted
    // session. Publication of the replacement must beat that completion.
    if score {
        tier1
            .edit(&gpu, &base, set, Instant::now())
            .map_err(Failure::Compile)?;
        std::thread::sleep(Duration::from_millis(405));
        if tier1.poll_promotion(&gpu).map_err(Failure::Device)? {
            return Err(Failure::Truth(
                "the superseded generation installed synchronously".into(),
            ));
        }
    }
    let next = tier1
        .edit(&gpu, &edited, set, Instant::now())
        .map_err(Failure::Compile)?;
    let quiet_start = Instant::now();
    if tier1.poll_promotion(&gpu).map_err(Failure::Device)? {
        return Err(Failure::Truth(
            "promotion installed inside the quiet period".into(),
        ));
    }
    let deadline = quiet_start + Duration::from_secs(30);
    while !tier1.poll_promotion(&gpu).map_err(Failure::Device)? {
        if Instant::now() > deadline {
            return Err(Failure::Device("trace promotion timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    let promotion = quiet_start.elapsed();
    let (creates, stale) = tier1.promotion_stats();
    println!(
        "promotion generation {} after {:.3} ms, background pipeline creates {creates}, stale completions discarded {stale}, cold edit elapsed {:.3} ms since upload",
        next.generation,
        millis(promotion),
        millis(uploaded.elapsed())
    );

    for _ in 0..WARMUP_FRAMES {
        tier0.loaded.frame(&gpu, t);
        tier1.loaded.frame(&gpu, t);
    }
    let a = tier0.loaded.pixels(&gpu).map_err(Failure::Device)?;
    let promoted = tier1.loaded.pixels(&gpu).map_err(Failure::Device)?;
    if let Some(max) = max_frame {
        budget::check(&gpu, &tier0.loaded, max)?;
    }
    let timing = if gpu.timestamps {
        Some(measure(&gpu, &tier0.loaded, &tier1.loaded, t, score)?)
    } else {
        None
    };

    let compile_start = Instant::now();
    let (fresh, _, _) =
        loft_passes::compile(&edited, set).map_err(|d| Failure::Compile(d.render(&edited)))?;
    let fresh_compile = millis(compile_start.elapsed());
    let loaded = Loaded::new(&gpu, &fresh, SIZE, None).map_err(Failure::Device)?;
    let k = tier0.kind();
    loaded.frame(&gpu, t);
    let b = loaded.pixels(&gpu).map_err(Failure::Device)?;
    let inputs = field_inputs(fresh.kinds[k].n_rows);
    let field0 = tier0
        .loaded
        .twin(&gpu, k, &inputs)
        .map_err(Failure::Device)?;
    let field1 = loaded.twin(&gpu, k, &inputs).map_err(Failure::Device)?;
    let error = field0
        .chunks_exact(16)
        .zip(field1.chunks_exact(16))
        .map(|(a, b)| {
            let delta = (a[0] - b[0]).abs();
            if delta.is_finite() {
                delta
            } else {
                f32::INFINITY
            }
        })
        .fold(0., f32::max);
    let bounds = bounds_match(&base, &edited, &fresh, kind, set)?;
    let changed = pixel_count(&before, &a, 0);
    let exact = pixel_count(&a, &b, 0);
    let over = pixel_count(&a, &b, 1);
    let promotion_diff = pixel_count(&promoted, &b, 0);
    let changed_pipelines = usize::from(clean0 != tier0.clean_pipelines())
        + usize::from(clean1 != tier1.clean_pipelines());
    println!(
        "fresh compile {fresh_compile:.6} ms, bounds exact {bounds}, changed pixels {changed}, exact RGBA differences {exact}, pixels over 1/255 {over}/{}, promoted/fresh differences {promotion_diff}, max field error {error:.9} over {} samples, clean-kind pipeline sets changed {changed_pipelines}",
        a.len() / 4,
        inputs.len() / 8
    );
    if score {
        trace_work(&gpu, &edited, set, t)?;
    }
    write_png(out, SIZE, &a).map_err(Failure::Usage)?;
    if !bounds
        || over * 1000 > a.len() / 4
        || error > 1e-5
        || changed_pipelines != 0
        || receipt.foreground_creates != 0
        || next.foreground_creates != 0
        || (score
            && (changed == 0
                || exact != 0
                || promotion_diff != 0
                || stale != 1
                || promotion < Duration::from_millis(398)))
    {
        return Err(Failure::Truth(
            "edit parity, generations or pipeline identity failed".into(),
        ));
    }
    if score
        && let Some([delta, same, triangle]) = timing
        && (delta > FRAME_MS
            || same > SAME_MS
            || triangle > TRIANGLE_MS
            || millis(receipt.elapsed) > EDIT_MS)
    {
        return Err(Failure::Budget(
            "edit or timestamp control exceeded its D2 budget".into(),
        ));
    }
    Ok(())
}

fn millis(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.
}

fn prepare(
    base: &Sources,
    kind: &str,
    set: &[(String, f64)],
) -> Result<(Program, TraceSource, loft_passes::tier0::TraceEdit), Failure> {
    let error = |d: loft_ir::Diag| Failure::Compile(d.render(base));
    let (program, _, checked) = loft_passes::compile(base, set).map_err(error)?;
    let (source, tape) = TraceSource::new(
        Sources {
            files: base.files.clone(),
        },
        checked,
        &program,
        kind,
    )
    .map_err(error)?;
    Ok((program, source, tape))
}

fn session(
    gpu: &Gpu,
    base: &Sources,
    kind: &str,
    set: &[(String, f64)],
) -> Result<TraceSession, Failure> {
    let (program, source, tape) = prepare(base, kind, set)?;
    TraceSession::new(gpu, &program, source, tape, SIZE).map_err(Failure::Device)
}

fn bounds_match(
    base: &Sources,
    edited: &Sources,
    fresh: &Program,
    kind: &str,
    set: &[(String, f64)],
) -> Result<bool, Failure> {
    let (_, source, _) = prepare(base, kind, set)?;
    let k = source.kind();
    let PreparedEdit {
        bounds,
        local_bounds,
        lips,
        ..
    } = source
        .prepare(edited, set)
        .map_err(|d| Failure::Compile(d.render(edited)))?;
    let table = &fresh.kinds[k];
    let world = fresh
        .instances
        .iter()
        .filter(|i| i.kind as usize == k)
        .map(|i| (i.row, i.lo, i.hi));
    let stride = table.stride as usize;
    let local = table
        .rows
        .chunks_exact(stride)
        .map(|row| &row[stride - abi::HIDDEN_WORDS as usize..]);
    let expected = local_bounds
        .iter()
        .zip(&lips)
        .map(|([lo, hi], &l)| [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], l]);
    Ok(bounds.iter().map(|i| (i.row, i.lo, i.hi)).eq(world)
        && local
            .map(|r| <[f32; 7]>::try_from(r).expect("trace hidden columns"))
            .eq(expected)
        && lips == table.lipschitz)
}

fn measure(gpu: &Gpu, a: &Loaded, b: &Loaded, t: f32, raw: bool) -> Result<[f64; 3], Failure> {
    let read = |l: &Loaded| {
        l.gpu_ms(gpu)
            .map_err(Failure::Device)?
            .ok_or_else(|| Failure::Device("frame has no timestamp endpoints".into()))
    };
    let mut diffs = Vec::with_capacity(TIMING_FRAMES);
    let mut pass_samples = Vec::with_capacity(TIMING_FRAMES);
    for i in 0..TIMING_FRAMES {
        let [first, second] = if i % 2 == 0 { [a, b] } else { [b, a] };
        first.frame(gpu, t);
        second.frame(gpu, t);
        let first = read(first)?;
        let second = read(second)?;
        let [(a, ap), (b, bp)] = if i % 2 == 0 {
            [first, second]
        } else {
            [second, first]
        };
        diffs.push(a - b);
        let d = ap.iter().zip(&bp).map(|(a, b)| a - b).collect::<Vec<_>>();
        if raw {
            println!(
                "AB {i:03} first_tier {} frames {a:.6},{b:.6} passes {ap:.6?},{bp:.6?}",
                i % 2
            );
        }
        pass_samples.push(d);
    }
    let summarize = |q| {
        (0..pass_samples[0].len())
            .map(|pass| quantile(&pass_samples.iter().map(|v| v[pass]).collect::<Vec<_>>(), q))
            .collect::<Vec<_>>()
    };
    let p95 = quantile(&diffs, 0.95);
    println!(
        "{TIMING_FRAMES} paired GPU frames, paired p50 {:.6} ms, paired p95 overhead {p95:.6} ms",
        quantile(&diffs, 0.50)
    );
    println!(
        "paired pass delta p50 {:.6?}, p95 {:.6?} ms in schedule order",
        summarize(0.50),
        summarize(0.95)
    );
    if !raw {
        return Ok([p95, 0., 0.]);
    }

    let aa = b
        .profile_frames(gpu, std::iter::repeat_n(t, 2 * TIMING_FRAMES), true)
        .map_err(Failure::Device)?;
    let mut same = Vec::with_capacity(TIMING_FRAMES);
    for (i, pair) in aa.chunks_exact(2).enumerate() {
        let d = (pair[0].gpu_ms - pair[1].gpu_ms) * if i % 2 == 0 { 1. } else { -1. };
        same.push(d);
        println!(
            "AA {i:03} {d:+.6} ms raw ticks {:?} {:?}",
            pair[0].ticks, pair[1].ticks
        );
    }
    let a_raw = a
        .profile_frames(gpu, [t; 4], true)
        .map_err(Failure::Device)?;
    for frame in &a_raw {
        println!(
            "Tier 0 endpoint sample {} commands {} raw ticks {:?}",
            frame.serial, frame.commands, frame.ticks
        );
    }
    let triangle = (0..3)
        .map(|_| gpu.triangle_control_ms(SIZE).map_err(Failure::Device))
        .collect::<Result<Vec<_>, _>>()?;
    let triangle_median = quantile(&triangle, 0.5);
    let same_p95 = quantile(&same, 0.95);
    println!(
        "AA paired p95 {same_p95:.6} ms, triangle control medians {triangle:.6?} ms, median {triangle_median:.6} ms"
    );
    Ok([p95, same_p95, triangle_median])
}

fn trace_work(gpu: &Gpu, src: &Sources, set: &[(String, f64)], t: f32) -> Result<(), Failure> {
    use loft_passes::emit::Aov::SdfSteps;
    let (program, _, _) = loft_passes::compile_aov(src, set, SdfSteps)
        .map_err(|d| Failure::Compile(d.render(src)))?;
    let loaded = Loaded::with_aov(gpu, &program, SIZE, None, SdfSteps).map_err(Failure::Device)?;
    loaded.frame(gpu, t);
    let pixels = loaded.pixels(gpu).map_err(Failure::Device)?;
    let width = SIZE.0 as usize;
    let mut tiles = vec![false; (SIZE.0.div_ceil(8) * SIZE.1.div_ceil(8)) as usize];
    let mut count = 0u64;
    let mut total = 0u64;
    let mut max = 0u64;
    for (i, rgba) in pixels.chunks_exact(4).enumerate() {
        let [r, g, b] = [rgba[0], rgba[1], rgba[2]].map(f64::from);
        if (r + g + b - 255.).abs() > 2. || (r != 0. && b != 0.) {
            continue;
        }
        let steps = (96. + (r - b) * (96. / 255.)).round() as u64;
        count += 1;
        total += steps;
        max = max.max(steps);
        tiles[(i / width / 8) * width.div_ceil(8) + i % width / 8] = true;
    }
    println!(
        "fresh SdfSteps hits {count}, traced shade tiles {}, hit march visits {total}, max {max}, shade visits {}",
        tiles.into_iter().filter(|v| *v).count(),
        count * 9
    );
    Ok(())
}

fn field_inputs(rows: u32) -> Vec<f32> {
    let mut result = Vec::new();
    for row in 0..rows {
        for z in -6..=6 {
            for y in -6..=6 {
                for x in -6..=6 {
                    result.extend([
                        x as f32 * 0.21,
                        y as f32 * 0.22,
                        z as f32 * 0.23,
                        f32::from_bits(row),
                        0.,
                        0.,
                        1.,
                        0.,
                    ]);
                }
            }
        }
    }
    result
}

fn pixel_count(a: &[u8], b: &[u8], limit: u8) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > limit))
        .count()
}
#[cfg(test)]
mod tests {
    use super::*;
    use loft_passes::tier0::EditClass;

    #[test]
    fn d1_edit_checks_the_source_dependency_slice_and_residual() {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        let fixture = here.join("tests/d1_edit");
        let old = fixture.join("literal-old.loft");
        let new = fixture.join("literal-new.loft");
        let sources = crate::sources(&old).unwrap();
        let (program, _, checked) = loft_passes::compile(&sources, &[]).unwrap();
        let (session, tape) = TraceSource::new(sources, checked, &program, "EditSphere").unwrap();
        let shader = tape.shader("").unwrap();
        let module = naga::front::wgsl::parse_str(&shader)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&shader)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&shader)));
        let edited = crate::sources(&new).unwrap();
        let literal = session
            .prepare(&edited, &[])
            .unwrap_or_else(|d| panic!("{}", d.render(&edited)));
        assert_eq!(literal.class(&tape), EditClass::ConstantPool);
        assert!(
            literal
                .tape
                .describe()
                .contains("opcodes [3, 2, 9, 1, 42, 55], live vec4 registers 2")
        );
        assert_eq!(literal.bounds[0].hi, [0.6886375; 3]);
        println!(
            "generation 0 {} → generation 1 {}, dirtied rows [0], bound {:?}",
            tape.describe(),
            literal.tape.describe(),
            literal.bounds[0].hi
        );
        let edited = crate::sources(&fixture.join("structural.loft")).unwrap();
        let structural = session
            .prepare(&edited, &[])
            .unwrap_or_else(|d| panic!("{}", d.render(&edited)));
        assert_eq!(structural.class(&literal.tape), EditClass::StructuralTape);
        assert!(structural.tape.describe().starts_with("Box"));
        println!("generation 2 {}", structural.tape.describe());
        let edited_loop = crate::sources(&fixture.join("loop.loft")).unwrap();
        let loop_edit = session
            .prepare(&edited_loop, &[])
            .unwrap_or_else(|d| panic!("{}", d.render(&edited_loop)));
        assert!(loop_edit.tape.describe().contains("53"));
        assert!(loop_edit.tape.describe().contains("54"));
        println!("bounded authored loop {}", loop_edit.tape.describe());
        let bad = crate::sources(&fixture.join("rejected.loft")).unwrap();
        let err = session
            .prepare(&bad, &[])
            .err()
            .expect("unbound variable must be rejected");
        let diagnostic = err.render(&bad);
        assert!(diagnostic.contains("E011") && diagnostic.contains(":2:32"));
        assert!(diagnostic.contains("unbound_edit_name"));
        println!("{diagnostic}");
        assert_eq!(
            session.prepare(&edited, &[]).unwrap().tape.describe(),
            structural.tape.describe()
        );
    }

    #[test]
    fn d2b_fuses_unseen_trees_and_keeps_the_full_pressure_residual() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/d1_edit");
        for (base, edit) in [
            ("literal-old", "clip"),
            ("literal-old", "reverse"),
            ("literal-old", "slab"),
            ("literal-old", "union"),
            ("nested-old", "nested"),
            ("full-old", "full"),
        ] {
            let base = crate::sources(&fixture.join(format!("{base}.loft"))).unwrap();
            let edited = crate::sources(&fixture.join(format!("{edit}.loft"))).unwrap();
            let (_, session, _) = prepare(&base, "EditSphere", &[]).unwrap();
            let descriptor = session
                .prepare(&edited, &[])
                .unwrap_or_else(|e| panic!("{}", e.render(&edited)))
                .tape
                .describe();
            let fresh = loft_passes::compile(&edited, &[]).unwrap().0;
            assert!(
                bounds_match(&base, &edited, &fresh, "EditSphere", &[]).unwrap(),
                "{edit}"
            );
            if edit == "full" {
                let opcode_list = descriptor
                    .split_once("opcodes [")
                    .unwrap()
                    .1
                    .split_once(']')
                    .unwrap()
                    .0;
                assert_eq!(opcode_list.split(", ").count(), 63);
                assert!(descriptor.starts_with("residual block PC"), "{descriptor}");
            } else {
                assert!(descriptor.starts_with("fused"), "{edit}: {descriptor}");
            }
            println!("{edit} {descriptor}");
        }
    }
}
