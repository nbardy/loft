//! Run the same fixed-step entry for interactive and baked sampling.
use super::*;
use loft_ir::{DepthConvention, GpuFeatures, HostCaps};
use loft_passes::{motion as fusion, program::MotionSample};
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

/// Check the scalar f32 columns that native sampling reads back. Callers can report a scene error
/// before allocating GPU resources.
pub fn motion_layout(p: &Program) -> Result<(), String> {
    p.dispatches.iter().try_for_each(|step| scalar_writes(p, step))
}

fn sample(g: &Gpu, p: &Program, seconds: f32, clock: Option<&Clock>) -> Result<(Vec<MotionSample>, f64), String> {
    let mut ms = 0.0;
    if p.dispatches.is_empty() { return Ok((vec![], ms)); }
    if !seconds.is_finite() || seconds < 0. || f64::from(seconds) * 120. >= f64::from(u32::MAX) {
        return Err(
            "motion time must be finite, nonnegative and representable in 120 Hz ticks".into(),
        );
    }
    motion_layout(p)?;
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
    let err = |e| format!("{e:?}");
    let mut samples: Vec<(usize, MotionSample)> = vec![];
    let mut advance = |runner: &mut Runner, id, active: bool| -> Result<(), String> {
        let mut encoder = g.device.create_command_encoder(&Default::default());
        let timed = clock.filter(|_| active);
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
        Ok(())
    };
    for (index, step) in p.dispatches.iter().enumerate().filter(|(_, step)| !step.lockstep) {
        let n_rows = step.input.view(&p.kinds).ok_or("dispatch input table is missing")?.1;
        let id = runner.load_bundle(&fusion::motion_bundle(p, step, 0)).map_err(err)?;
        advance(&mut runner, id, n_rows != 0)?;
        samples.push((index, collect(g, &runner, id, p, step, fusion::ROWS, Some(fusion::RESULTS))?));
    }
    // Kinds that read each other's rows advance together, one tick at a time, in one bundle.
    let group: Vec<usize> = (0..p.dispatches.len()).filter(|&i| p.dispatches[i].lockstep).collect();
    if !group.is_empty() {
        let (bundle, tables) = fusion::motion_group(p, &group);
        let id = runner.load_bundle(&bundle).map_err(err)?;
        advance(&mut runner, id, true)?;
        for (&index, table) in group.iter().zip(tables) {
            samples.push((index, collect(g, &runner, id, p, &p.dispatches[index], table.rows, table.results)?));
        }
    }
    samples.sort_by_key(|(index, _)| *index);
    Ok((samples.into_iter().map(|(_, sample)| sample).collect(), ms))
}

/// Stills sample a dispatch's writes as scalar f32 columns of its `Row`.
fn scalar_writes(p: &Program, step: &loft_passes::program::Dispatch) -> Result<(), String> {
    let module = &p.modules[step.module].naga;
    let row = module.types.iter()
        .find(|(_, ty)| ty.name.as_deref() == Some("Row"))
        .and_then(|(_, ty)| {
            let naga::TypeInner::Struct { members, .. } = &ty.inner else { return None };
            Some(members)
        })
        .ok_or("missing state row layout")?;
    for (field, _) in &step.fields {
        let name = format!("r_{field}");
        if !row.iter().any(|m| {
            m.name.as_deref() == Some(&name)
                && module.types[m.ty].inner == naga::TypeInner::Scalar(naga::Scalar::F32)
        }) {
            return Err(format!(
                "law writes `{field}`, which is not a scalar float: native motion sampling reads only \
                 scalar f32 law writes. Keep the vector field fixed and write scalars (e.g. `phase`, `x`) that \
                 the pose or trace reads"
            ));
        }
    }
    Ok(())
}

/// Read back one dispatch's written columns and verdicts after an advance.
fn collect(g: &Gpu, runner: &Runner, id: loft_ir::InstanceId, p: &Program, step: &loft_passes::program::Dispatch,
    rows: loft_ir::TableId, results: Option<loft_ir::TableId>) -> Result<MotionSample, String> {
    let (name, n_rows, stride, _) = step.input.view(&p.kinds).ok_or("dispatch input table is missing")?;
    let state = runner.export_buffer(id, rows).ok_or("missing state table")?;
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
    if let Some(result) = results.and_then(|table| runner.export_buffer(id, table)) {
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
    Ok(MotionSample { kind: name.into(), fields, checks })
}
