//! `loft still SCENE OUT.png --edit EDITED --kind K`: make an edit to one kind live by recompiling its
//! module, then check the live picture against a fresh load of the edited scene.
use crate::budget::{self, Failure};
use loft_cli::native::EditSession;
use loft_cli::write_png;
use loft_passes::program::Program;
use std::{
    path::Path,
    time::{Duration, Instant},
};

const SIZE: (u32, u32) = (1280, 800);

fn program(path: &Path, set: &[(String, f64)]) -> Result<Program, Failure> {
    let sources = crate::sources(path).map_err(Failure::Usage)?;
    loft_passes::compile(&sources, set).map(|(p, ..)| p).map_err(|d| Failure::Compile(d.render(&sources)))
}

pub fn edited_still(
    base: &Path,
    edit: &Path,
    kind: &str,
    set: &[(String, f64)],
    t: f32,
    out: &Path,
    max_frame: Option<f64>,
) -> Result<(), Failure> {
    let gpu = loft_cli::native::gpu().map_err(Failure::Device)?;
    if max_frame.is_some() {
        budget::timestamps(gpu.timestamps)?;
    }
    let mut session = EditSession::new(&gpu, program(base, set)?, kind, SIZE).map_err(Failure::Device)?;
    session.loaded.frame(&gpu, t);
    let before = session.loaded.pixels(&gpu).map_err(Failure::Device)?;

    let edited = crate::sources(edit).map_err(Failure::Usage)?;
    let receipt = session.edit(&gpu, &edited, set, Instant::now()).map_err(Failure::Compile)?;
    let ms = |d: Duration| d.as_secs_f64() * 1000.;
    println!(
        "edit live in {:.3} ms: compile {:.3} ms, pipelines {:.3} ms, upload {:.3} ms; dirtied rows {:?}, caster bound changed {}",
        ms(receipt.elapsed),
        ms(receipt.compile),
        ms(receipt.pipelines),
        ms(receipt.upload),
        receipt.dirtied_rows,
        receipt.caster_changed
    );
    session.loaded.frame(&gpu, t);
    let after = session.loaded.pixels(&gpu).map_err(Failure::Device)?;
    if let Some(max) = max_frame {
        budget::check(&gpu, &session.loaded, max)?;
    }

    let fresh = EditSession::new(&gpu, program(edit, set)?, kind, SIZE).map_err(Failure::Device)?;
    fresh.loaded.frame(&gpu, t);
    let reference = fresh.loaded.pixels(&gpu).map_err(Failure::Device)?;
    let changed = pixel_count(&before, &after, 0);
    let over = pixel_count(&after, &reference, 1);
    println!("changed pixels {changed}, pixels over 1/255 against a fresh load {over}/{}", after.len() / 4);
    write_png(out, SIZE, &after).map_err(Failure::Usage)?;
    if over != 0 {
        return Err(Failure::Truth("the live edit differs from a fresh load of the edited scene".into()));
    }
    Ok(())
}

fn pixel_count(a: &[u8], b: &[u8], limit: u8) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(a, b)| a[..3].iter().zip(&b[..3]).any(|(x, y)| x.abs_diff(*y) > limit))
        .count()
}
