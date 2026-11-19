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
        // The runner can split a tick range across passes. Bracket the whole
        // encoded advance with portable pass-boundary timestamps before readback.
        let timestamp = |enc: &mut wgpu::CommandEncoder, begin, end| {
            if let Some(c) = clock {
                // Metal can discard empty compute passes. Stamp the encoder boundary when supported.
                if g.device.features().contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS) {
                    enc.write_timestamp(&c.set, begin.or(end).expect("timestamp index"));
                    return;
                }
                drop(enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("motion timing boundary"),
                    timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                        query_set: &c.set,
                        beginning_of_pass_write_index: begin,
                        end_of_pass_write_index: end,
                    }),
                }));
            }
        };
        timestamp(&mut encoder, None, Some(0));
        runner
            .advance_into(id, &mut encoder, ticks, 0)
            .map_err(err)?;
        timestamp(&mut encoder, Some(1), None);
        g.queue.submit([encoder.finish()]);
        if let Some(c) = clock {
            let stamps = read_clock(g, c)?;
            let dt = stamps[1].checked_sub(stamps[0]).filter(|_| stamps[0] != 0)
                .ok_or("motion GPU timestamps are unavailable")?;
            ms += dt as f64 * c.period_ns / 1e6;
        }
        let state = runner
            .export_buffer(id, fusion::ROWS)
            .ok_or("missing state table")?;
        let read = |buffer: &wgpu::Buffer| -> Result<Vec<u32>, String> {
            use wgpu::BufferUsages as U;
            let staging = g.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: buffer.size(),
                usage: U::COPY_DST | U::MAP_READ,
                mapped_at_creation: false,
            });
            let mut enc = g.device.create_command_encoder(&Default::default());
            enc.copy_buffer_to_buffer(buffer, 0, &staging, 0, Some(staging.size()));
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            enc.map_buffer_on_submit(&staging, wgpu::MapMode::Read, .., move |r| {
                let _ = tx.send(r);
            });
            g.queue.submit([enc.finish()]);
            g.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| e.to_string())?;
            rx.recv()
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            let mapped = staging.get_mapped_range(..).map_err(|e| e.to_string())?;
            Ok(mapped
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect())
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
