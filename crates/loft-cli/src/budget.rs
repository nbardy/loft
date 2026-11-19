//! Frame budgets use device timestamps, including when running without a window.
use loft_runner::{Gpu, Loaded};
use std::path::Path;

pub struct Failure(pub u8, pub String);

pub fn timestamps(available: bool) -> Result<(), Failure> {
    if available {
        Ok(())
    } else {
        Err(Failure(
            6,
            "adapter does not support timestamp queries required for a GPU frame budget".into(),
        ))
    }
}

pub fn check(g: &Gpu, scene: &Path, set: &[(String, f64)], budget: f64) -> Result<(), Failure> {
    let src = crate::sources(scene).map_err(|e| Failure(5, e))?;
    let (program, _, _) =
        loft_passes::compile(&src, set).map_err(|d| Failure(1, d.render(&src)))?;
    let loaded = Loaded::new(g, &program, (1280, 800), None).map_err(|e| Failure(6, e))?;
    let mut samples = vec![];
    for i in 0..90 {
        loaded.frame(g, i as f32 / 60.);
        let (ms, _) = loaded
            .gpu_ms(g)
            .map_err(|e| Failure(6, e))?
            .ok_or_else(|| Failure(6, "GPU timestamps are unavailable".into()))?;
        if i >= 30 {
            samples.push(ms);
        }
    }
    samples.sort_by(f64::total_cmp);
    let p95 = samples[((samples.len() - 1) as f64 * 0.95).round() as usize];
    if p95 > budget {
        Err(Failure(
            7,
            format!("GPU p95 {p95:.3} ms exceeds {budget:.3} ms"),
        ))
    } else {
        println!("GPU p95 {p95:.3} ms <= {budget:.3} ms");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_timestamps_are_an_environment_failure() {
        let failure = super::timestamps(false).err().unwrap();
        assert_eq!(failure.0, 6);
    }
}
