//! Measure signed margins stored by a presented table. The predicate and margin refer to words of the *same* record.
use crate::{Verdict, gpu, load, read_bundle};
use loft_core::check::{Evaluator, Truth, observed};
use loft_ir::{
    bundle::Shape,
    identity::{Precision, build},
};
use std::path::Path;

#[derive(serde::Deserialize)]
struct Site {
    label: String,
    word: usize,
    negative: Option<usize>,
}

pub(super) fn run(dir: &Path, index: usize, sites: &str) -> Result<(), Verdict> {
    let sites: Vec<Site> = serde_json::from_str(sites).map_err(|e| Verdict::Usage(format!("trace sites: {e}")))?;
    let g = gpu(false)?;
    let mut l = load(&g, &read_bundle(dir)?, Precision::F32, None)?;
    let b = l.bundle();
    let s =
        b.row_steps.get(index).ok_or_else(|| Verdict::Usage("trace needs a declared stepped table".into()))?.clone();
    let Shape::Rows { rows, lanes } = b.resources[s.output as usize].shape else { unreachable!() };
    let words = rows as usize * lanes as usize * 4;
    if sites.is_empty() || sites.iter().any(|x| x.word >= words || x.negative.is_some_and(|n| n >= words)) {
        return Err(Verdict::Usage(format!(
            "declare a margin and optional negative-mode word inside the table's {words} floats"
        )));
    }
    let law = build(&b.sections[b.modules[s.module as usize].wgsl as usize]);
    let mut samples = vec![vec![]; sites.len()];
    // Bound staging memory. A take's length does not set the size of a GPU allocation.
    for first in (0..=s.ticks).step_by(32) {
        let last = s.ticks.min(first + 31);
        let stride = words as u64 * 4;
        let size = stride * u64::from(last - first + 1);
        if size > g.device.limits().max_buffer_size {
            return Err(Verdict::Usage("trace staging exceeds this device: inspect fewer rows".into()));
        }
        let dst = g.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("observed rows"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = g.device.create_command_encoder(&Default::default());
        for k in first..=last {
            l.set_time(k as f32 / s.hz as f32);
            l.advance_rows(&mut enc);
            l.copy_rows(&mut enc, s.output, &dst, u64::from(k - first) * stride)?;
        }
        g.queue.submit([enc.finish()]);
        dst.map_async(wgpu::MapMode::Read, .., |_| {});
        g.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| Verdict::Environment(e.to_string()))?;
        let bytes = dst.get_mapped_range(..).map_err(|e| Verdict::Environment(e.to_string()))?;
        for row in bytes.chunks_exact(stride as usize) {
            let value = |i: usize| f32::from_le_bytes(row[4 * i..4 * i + 4].try_into().unwrap());
            for (site, margins) in sites.iter().zip(&mut samples) {
                let mode = site.negative.map(value);
                if mode.is_some_and(|m| !m.is_finite()) {
                    return Err(Verdict::Scene(format!(
                        "{} has an invalid mode: supply a finite predicate",
                        site.label
                    )));
                }
                margins.push(if mode.is_none_or(|m| m < 0.0) { Some(value(site.word)) } else { None });
            }
        }
    }
    let prefix = l.frame_key(0, [1, 1]).steps[index].prefix;
    let mut failed = 0;
    for (site, margins) in sites.into_iter().zip(samples) {
        let v = observed(&margins, prefix, law, Evaluator::Gpu)?;
        failed += usize::from(!matches!(v.truth, Truth::Holds));
        println!(
            "{}",
            serde_json::json!({"label":site.label,"observations":margins.iter().flatten().count(),"verdict":v})
        );
    }
    if failed == 0 { Ok(()) } else { Err(Verdict::Check(format!("{failed} traced margins failed"))) }
}

/// Expose the emitted module, including the generated dispatch. No device is needed for inspection.
pub(super) fn modules(path: &Path) -> Result<(), Verdict> {
    use loft_ir::bundle::{Bundle, InputKind};
    let b = Bundle::read(&read_bundle(path)?)?;
    let invalid = || Verdict::Load("module reference out of bounds: rebuild the bundle".into());
    let name = |id: u32| b.names.get(id as usize).map(AsRef::as_ref).ok_or_else(invalid);
    for (i, m) in b.modules.iter().enumerate() {
        let label = if let Some(step) = b.row_steps.iter().find(|s| s.module as usize == i) {
            let input = b.inputs.get(step.input as usize).ok_or_else(invalid)?;
            let home = match input.kind {
                InputKind::Level | InputKind::Events { .. } => "live",
                InputKind::Script { .. } => "baked",
            };
            format!("{home} {} · {} ticks at {} Hz", name(input.name)?, step.ticks, step.hz)
        } else {
            let draws = b
                .draws
                .iter()
                .filter(|d| d.shade.0 as usize == i)
                .map(|d| name(d.name))
                .collect::<Result<Vec<&str>, _>>()?;
            if draws.is_empty() { format!("module {i}") } else { draws.join(" ") }
        };
        let section = b.sections.get(m.wgsl as usize).ok_or_else(invalid)?;
        let wgsl = std::str::from_utf8(section).map_err(|e| Verdict::Load(e.to_string()))?;
        let text = format!("{}{wgsl}", loft_gpu::prelude(m, Precision::F32));
        println!("{}", serde_json::json!({"label":label,"text":text,"lines":text.lines().count(),"sited":0}));
    }
    Ok(())
}
