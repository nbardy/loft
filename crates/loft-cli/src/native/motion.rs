//! Run the same fixed-step entry for interactive and baked sampling.
use super::*;
use loft_ir::{DepthConvention, GpuFeatures, HostCaps, InstanceId};
use loft_passes::{
    fusion,
    program::{Dispatch as MotionDispatch, DispatchResource, MotionSample},
};
use loft_runner::Runner;
use std::sync::Arc;

pub fn sample_motion(g: &Gpu, p: &Program, seconds: f32) -> Result<Vec<MotionSample>, String> {
    Ok(sample(g, p, seconds, None)?.0)
}

/// The same submitted law/check dispatches, timed in GPU milliseconds for clip scoring.
pub fn sample_motion_timed(
    g: &Gpu,
    p: &Program,
    seconds: f32,
) -> Result<(Vec<MotionSample>, f64), String> {
    let clock = (g.timestamps && !p.dispatches.is_empty()).then(|| Clock::new(g, 2));
    sample(g, p, seconds, clock.as_ref())
}

fn sample(
    g: &Gpu,
    p: &Program,
    seconds: f32,
    clock: Option<&Clock>,
) -> Result<(Vec<MotionSample>, f64), String> {
    if p.dispatches.is_empty() {
        return Ok((vec![], 0.));
    }
    if !seconds.is_finite() || seconds < 0. || f64::from(seconds) * 120. >= f64::from(u32::MAX) {
        return Err(
            "motion time must be finite, nonnegative and representable in 120 Hz ticks".into(),
        );
    }
    let tick = (f64::from(seconds) * 120.).round() as u32;
    let mut session = g.motion_session(p)?;
    let ms = session.advance(tick, clock)?;
    Ok((session.samples()?, ms))
}

pub struct MotionSession<'a> {
    gpu: &'a Gpu,
    runner: Runner,
    dispatches: Vec<LoadedDispatch<'a>>,
}

struct LoadedDispatch<'a> {
    id: InstanceId,
    step: &'a MotionDispatch,
    name: &'a str,
    rows: usize,
    stride: usize,
}

impl Gpu {
    /// Load each dispatch at its initial rows for repeated fixed-step advances.
    pub fn motion_session<'a>(&'a self, program: &'a Program) -> Result<MotionSession<'a>, String> {
        let mut runner = Runner::attach(
            Arc::new(self.device.clone()),
            Arc::new(self.queue.clone()),
            HostCaps {
                depth_convention: DepthConvention::StandardZeroToOne,
                msaa_samples: 1,
                features: GpuFeatures::empty(),
            },
        );
        let mut dispatches = Vec::with_capacity(program.dispatches.len());
        for step in &program.dispatches {
            let (name, rows, stride, _) = step
                .input
                .view(&program.kinds)
                .ok_or("dispatch input table is missing")?;
            let module = &program.modules[step.module].naga;
            let row = module
                .types
                .iter()
                .find_map(|(_, ty)| match &ty.inner {
                    naga::TypeInner::Struct { members, .. } if ty.name.as_deref() == Some("Row") => {
                        Some(members)
                    }
                    _ => None,
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
                .load_bundle(&fusion::motion_bundle(program, step, 0))
                .map_err(|e| format!("{e:?}"))?;
            dispatches.push(LoadedDispatch {
                id,
                step,
                name,
                rows: rows as usize,
                stride: stride as usize,
            });
        }
        Ok(MotionSession {
            gpu: self,
            runner,
            dispatches,
        })
    }
}

impl MotionSession<'_> {
    pub fn set_param(&mut self, name: &str, value: f32) -> Result<(), String> {
        for dispatch in &self.dispatches {
            let id = self
                .runner
                .params(dispatch.id)
                .iter()
                .find(|p| p.name == name)
                .ok_or_else(|| format!("missing motion parameter metadata for {name}"))?
                .id;
            self.runner
                .set_param(dispatch.id, id, &[value])
                .map_err(|e| format!("{e:?}"))?;
        }
        Ok(())
    }

    /// Submit every dispatch through the absolute tick before the next parameter edit.
    pub fn advance_to(&mut self, tick: u32) -> Result<(), String> {
        self.advance(tick, None)?;
        for dispatch in &self.dispatches {
            if dispatch
                .step
                .bindings
                .iter()
                .any(|b| b.resource == DispatchResource::Outbox)
            {
                // Replay discards output events, but their staging ring still needs completed reads.
                self.gpu
                    .device
                    .poll(wgpu::PollType::wait_indefinitely())
                    .map_err(|e| e.to_string())?;
                self.runner
                    .drain_events(dispatch.id)
                    .map_err(|e| format!("{e:?}"))?;
            }
        }
        Ok(())
    }

    fn advance(&mut self, tick: u32, clock: Option<&Clock>) -> Result<f64, String> {
        let mut ms = 0.;
        for dispatch in &self.dispatches {
            let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
            let timed = clock.filter(|_| dispatch.rows != 0);
            match timed {
                Some(c) => self.runner.advance_timed_into(dispatch.id, &mut encoder, tick, 0, &c.set),
                None => self.runner.advance_into(dispatch.id, &mut encoder, tick, 0),
            }
            .map_err(|e| format!("{e:?}"))?;
            self.gpu.queue.submit([encoder.finish()]);
            if let Some(c) = timed {
                let stamps = read_clock(self.gpu, c)?;
                let dt = stamps[1]
                    .checked_sub(stamps[0])
                    .filter(|_| stamps[0] != 0)
                    .ok_or("motion GPU timestamps are unavailable")?;
                ms += dt as f64 * c.period_ns / 1e6;
            }
        }
        Ok(ms)
    }

    pub fn samples(&self) -> Result<Vec<MotionSample>, String> {
        self.dispatches
            .iter()
            .map(|dispatch| {
                let state = self
                    .runner
                    .export_buffer(dispatch.id, fusion::ROWS)
                    .ok_or("missing state table")?;
                let state = read_words(self.gpu, &state)?;
                let fields = dispatch
                    .step
                    .fields
                    .iter()
                    .map(|(name, offset)| {
                        let values = state
                            .chunks_exact(dispatch.stride)
                            .take(dispatch.rows)
                            .map(|r| f32::from_bits(r[*offset as usize]))
                            .collect();
                        (name.clone(), values)
                    })
                    .collect();
                let mut checks = vec![];
                if let Some(result) = self.runner.export_buffer(dispatch.id, fusion::RESULTS) {
                    let records = read_words(self.gpu, &result)?;
                    for (index, name) in dispatch.step.checks.iter().enumerate() {
                        let values = records
                            .chunks_exact(dispatch.step.checks.len() * 8)
                            .take(dispatch.rows)
                            .map(|r| [r[index * 8 + 4] as f32, f32::from_bits(r[index * 8 + 6])])
                            .collect();
                        checks.push((name.clone(), values));
                    }
                }
                Ok(MotionSample {
                    kind: dispatch.name.into(),
                    fields,
                    checks,
                })
            })
            .collect()
    }
}

fn read_words(gpu: &Gpu, buffer: &wgpu::Buffer) -> Result<Vec<u32>, String> {
    use wgpu::BufferUsages as U;
    let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: buffer.size(),
        usage: U::COPY_DST | U::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, Some(staging.size()));
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    encoder.map_buffer_on_submit(&staging, wgpu::MapMode::Read, .., move |r| {
        let _ = tx.send(r);
    });
    gpu.queue.submit([encoder.finish()]);
    gpu.device
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
}
