use crate::{
    budget::{self, Failure},
    gates::{quantile, write_png},
};
use loft_cli::native::{Gpu, Loaded, TraceSession};
use loft_passes::tier0::TraceSource;
use std::{
    path::Path,
    time::{Duration, Instant},
};

const SIZE: (u32, u32) = (1280, 800);
const TIMING_FRAMES: usize = 120;
const WARMUP_FRAMES: usize = 12;

pub fn edited_still(
    base: &Path,
    edit: &Path,
    kind: &str,
    set: &[(String, f64)],
    t: f32,
    out: &Path,
    max_frame: Option<f64>,
) -> Result<(), Failure> {
    let sources = crate::sources(base).map_err(Failure::Usage)?;
    let (program, _, checked) =
        loft_passes::compile(&sources, set).map_err(|d| Failure::Compile(d.render(&sources)))?;
    let initial_error = |d: loft_ir::Diag| Failure::Compile(d.render(&sources));
    let (source, tape) = TraceSource::new(
        loft_ir::Sources {
            files: sources.files.clone(),
        },
        checked,
        &program,
        kind,
    )
    .map_err(initial_error)?;
    let gpu = loft_cli::native::gpu().map_err(Failure::Device)?;
    if max_frame.is_some() {
        budget::timestamps(gpu.timestamps)?;
    }
    let mut session =
        TraceSession::new(&gpu, &program, source, tape, SIZE).map_err(Failure::Device)?;
    let clean = session.clean_pipelines();
    session.loaded.frame(&gpu, t);
    let before = session.loaded.pixels(&gpu).map_err(Failure::Device)?;

    let edited = crate::sources(edit).map_err(Failure::Usage)?;
    let received = Instant::now();
    let edit = session
        .edit(&gpu, &edited, set, received)
        .map_err(Failure::Compile)?;
    let ms = |d: Duration| d.as_secs_f64() * 1000.;
    println!(
        "{:?} generation {}: {}",
        edit.class,
        edit.generation,
        session.describe()
    );
    println!(
        "dirtied rows {:?}, tape byte ranges {:?}, caster bound changed {}",
        edit.dirtied_rows, edit.tape_writes, edit.caster_changed
    );
    println!(
        "source-to-upload {:.6} ms, parse/check {:.6} ms, bounds/lowering {:.6} ms, queue upload {:.6} ms, foreground pipeline creations {}",
        ms(edit.elapsed),
        ms(edit.parse_check),
        ms(edit.bounds_lowering),
        ms(edit.upload),
        edit.foreground_creates
    );

    session.loaded.frame(&gpu, t);
    let tier0 = session.loaded.pixels(&gpu).map_err(Failure::Device)?;
    if let Some(max) = max_frame {
        budget::check(&gpu, &session.loaded, max)?;
    }
    let timing = timed_frames(&session.loaded, &gpu, t)?;
    let inputs = field_inputs();
    let field0 = session
        .loaded
        .twin(&gpu, session.kind(), &inputs)
        .map_err(Failure::Device)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while !session.poll_promotion(&gpu).map_err(Failure::Device)? {
        if Instant::now() > deadline {
            return Err(Failure::Device("trace promotion timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    session.loaded.frame(&gpu, t);
    let tier1 = session.loaded.pixels(&gpu).map_err(Failure::Device)?;
    let promoted = timed_frames(&session.loaded, &gpu, t)?;
    let field1 = session
        .loaded
        .twin(&gpu, session.kind(), &inputs)
        .map_err(Failure::Device)?;
    let (creates, stale) = session.promotion_stats();
    let clean_changed = clean
        .iter()
        .zip(session.clean_pipelines())
        .filter(|(a, b)| *a != b)
        .count();
    let changed = pixel_count(&before, &tier0, 0);
    let differing = pixel_count(&tier0, &tier1, 0);
    let over = pixel_count(&tier0, &tier1, 1);
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
    println!(
        "changed pixels {changed}, pixels over 1/255 after promotion {over}/{}, max field error {error:.9} over {} samples",
        tier0.len() / 4,
        inputs.len() / 8
    );
    println!("differing RGBA pixels after promotion {differing}");
    println!(
        "promotion created {creates} pipelines, stale completions discarded {stale}, clean-kind pipeline pairs changed {clean_changed}"
    );
    if !timing.is_empty() && !promoted.is_empty() {
        let a = quantile(&timing, 0.95);
        let b = quantile(&promoted, 0.95);
        println!(
            "GPU p95 Tier 0 {a:.6} ms, Tier 1 {b:.6} ms, overhead {:.6} ms",
            a - b
        );
        let paired = session
            .measure_paired(&gpu, t, TIMING_FRAMES)
            .map_err(Failure::Device)?;
        if let Some((_, passes)) = paired.first() {
            let delta: Vec<_> = paired.iter().map(|(ms, _)| *ms).collect();
            println!(
                "{} paired GPU frames, paired p95 overhead {:.6} ms",
                delta.len(),
                quantile(&delta, 0.95)
            );
            let pass_p95: Vec<_> = (0..passes.len())
                .map(|i| {
                    quantile(
                        &paired.iter().map(|(_, pass)| pass[i]).collect::<Vec<_>>(),
                        0.95,
                    )
                })
                .collect();
            println!("paired pass p95 overhead in schedule order {pass_p95:.6?} ms");
        }
    }
    write_png(out, SIZE, &tier0).map_err(Failure::Usage)?;
    if over * 1000 > tier0.len() / 4 || error > 1e-5 || clean_changed != 0 {
        return Err(Failure::Truth(
            "trace tape differs from the specialized render or changed a clean pipeline".into(),
        ));
    }
    Ok(())
}

fn timed_frames(loaded: &Loaded, gpu: &Gpu, t: f32) -> Result<Vec<f64>, Failure> {
    let mut samples = Vec::new();
    for i in 0..WARMUP_FRAMES + TIMING_FRAMES {
        loaded.frame(gpu, t);
        if let Some((ms, _)) = loaded.gpu_ms(gpu).map_err(Failure::Device)?
            && i >= WARMUP_FRAMES
        {
            samples.push(ms);
        }
    }
    Ok(samples)
}

fn field_inputs() -> Vec<f32> {
    let mut result = Vec::new();
    for z in -6..=6 {
        for y in -6..=6 {
            for x in -6..=6 {
                result.extend([
                    x as f32 * 0.21,
                    y as f32 * 0.22,
                    z as f32 * 0.23,
                    0.,
                    0.,
                    0.,
                    1.,
                    0.,
                ]);
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
        let probes = here.join("../../out/meta/d1c/probes");
        let old = if probes.join("edit-old.loft").exists() {
            probes.join("edit-old.loft")
        } else {
            fixture.join("literal-old.loft")
        };
        let new = if probes.join("edit-new.loft").exists() {
            probes.join("edit-new.loft")
        } else {
            fixture.join("literal-new.loft")
        };
        let sources = crate::sources(&old).unwrap();
        let (program, _, checked) = loft_passes::compile(&sources, &[]).unwrap();
        let (session, tape) = TraceSource::new(sources, checked, &program, "EditSphere").unwrap();
        let edited = crate::sources(&new).unwrap();
        let verify_shader = |shader: &str| {
            naga::front::wgsl::parse_str(shader)
                .unwrap_or_else(|e| panic!("{}", e.emit_to_string(shader)));
        };
        verify_shader(&tape.shader("").unwrap());
        let literal = session
            .prepare(&edited, &[])
            .unwrap_or_else(|d| panic!("{}", d.render(&edited)));
        verify_shader(&literal.shader);
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
        assert!(err.render(&bad).contains("unbound_edit_name"));
        assert_eq!(
            session.prepare(&edited, &[]).unwrap().tape.describe(),
            structural.tape.describe()
        );
    }
}
