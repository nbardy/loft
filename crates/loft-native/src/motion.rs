//! Run the same fixed-step entry for interactive and baked sampling.
use super::*;
use loft_ir::{DepthConvention, GpuFeatures, HostCaps};
use loft_passes::{fusion, program::MotionSample};
use loft_runner::Runner;
use std::sync::Arc;

pub fn sample_motion(g: &Gpu, p: &Program, seconds: f32) -> Result<Vec<MotionSample>, String> {
    Ok(sample(g, p, seconds, None)?.0)
}

/// The same submitted law/check dispatches, timed in GPU milliseconds for clip scoring.
pub fn sample_motion_timed(g: &Gpu, p: &Program, seconds: f32) -> Result<(Vec<MotionSample>, f64), String> {
    let clock = (g.timestamps && !p.dispatches.is_empty()).then(|| Clock::new(g, 2));
    sample(g, p, seconds, clock.as_ref())
}

fn sample(g: &Gpu, p: &Program, seconds: f32, clock: Option<&Clock>) -> Result<(Vec<MotionSample>, f64), String> {
    let mut ms = 0.0;
    if p.dispatches.is_empty() { return Ok((vec![], ms)); }
    if !seconds.is_finite() || seconds < 0. || f64::from(seconds) * 120. >= f64::from(u32::MAX) {
        return Err(
            "motion time must be finite, nonnegative and representable in 120 Hz ticks".into(),
        );
    }
    let ticks = (f64::from(seconds) * 120.).round() as u32;
    let mut runner = Runner::attach(
        Arc::new(g.device.clone()),
        Arc::new(g.queue.clone()),
        HostCaps {
            depth_convention: DepthConvention::StandardZeroToOne,
            msaa_samples: 1,
            features: GpuFeatures::empty(),
        },
    );
    let mut samples = vec![];
    let err = |e| format!("{e:?}");
    for step in &p.dispatches {
        let (name, n_rows, stride, _) = step
            .input
            .view(&p.kinds)
            .ok_or("dispatch input table is missing")?;
        let module = &p.modules[step.module].naga;
        let row = module
            .types
            .iter()
            .find_map(|(_, ty)| {
                if ty.name.as_deref() == Some("Row")
                    && let naga::TypeInner::Struct { members, .. } = &ty.inner
                {
                    Some(members)
                } else {
                    None
                }
            })
            .ok_or("missing state row layout")?;
        for (field, _) in &step.fields {
            let name = format!("r_{field}");
            if !row.iter().any(|m| {
                m.name.as_deref() == Some(&name)
                    && module.types[m.ty].inner == naga::TypeInner::Scalar(naga::Scalar::F32)
            }) {
                return Err(format!(
                    "still sampling requires scalar float writes, found {field}"
                ));
            }
        }
        let id = runner
            .load_bundle(&fusion::motion_bundle(p, step, 0))
            .map_err(err)?;
        let mut encoder = g.device.create_command_encoder(&Default::default());
        let timed = clock.filter(|_| n_rows != 0);
        match timed {
            Some(c) => runner.advance_timed_into(id, &mut encoder, ticks, 0, &c.set),
            None => runner.advance_into(id, &mut encoder, ticks, 0),
        }.map_err(err)?;
        g.queue.submit([encoder.finish()]);
        if let Some(c) = timed {
            let (stamps, _) = read_clock(g, c)?;
            let dt = stamps[1].checked_sub(stamps[0]).filter(|_| stamps[0] != 0)
                .ok_or("motion GPU timestamps are unavailable")?;
            ms += dt as f64 * c.period_ns / 1e6;
        }
        let state = runner
            .export_buffer(id, fusion::ROWS)
            .ok_or("missing state table")?;
        let read = |buffer: &wgpu::Buffer| -> Result<Vec<u32>, String> {
            let bytes = loft_runner::read_buffer(&g.device, &g.queue, buffer).map_err(runner_error)?;
            Ok(bytes.as_chunks::<4>().0.iter().copied().map(u32::from_le_bytes).collect())
        };
        let state = read(&state)?;
        let fields = step
            .fields
            .iter()
            .map(|(name, offset)| {
                let values = state
                    .chunks_exact(stride as usize)
                    .take(n_rows as usize)
                    .map(|r| f32::from_bits(r[*offset as usize]))
                    .collect();
                (name.clone(), values)
            })
            .collect();
        let mut checks = vec![];
        if let Some(result) = runner.export_buffer(id, fusion::RESULTS) {
            let records = read(&result)?;
            for (index, name) in step.checks.iter().enumerate() {
                let values = records
                    .chunks_exact(step.checks.len() * 8)
                    .take(n_rows as usize)
                    .map(|r| [r[index * 8 + 4] as f32, f32::from_bits(r[index * 8 + 6])])
                    .collect();
                checks.push((name.clone(), values));
            }
        }
        samples.push(MotionSample {
            kind: name.into(),
            fields,
            checks,
        });
    }
    Ok((samples, ms))
}
