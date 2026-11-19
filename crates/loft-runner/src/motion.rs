//! Execute the compiler's row-local fixed-tick entry for a still or a semantic probe.
use super::*;
use loft_ir::program::MotionSample;

pub fn sample_motion(g: &Gpu, p: &Program, seconds: f32) -> Result<Vec<MotionSample>, String> {
    if !seconds.is_finite() || seconds < 0. || f64::from(seconds) * 120. >= f64::from(u32::MAX) {
        return Err(
            "motion time must be finite, nonnegative and representable in 120 Hz ticks".into(),
        );
    }
    let ticks = (f64::from(seconds) * 120.).round() as u32;
    let d = &g.device;
    use wgpu::BufferUsages as U;
    let empty = layout(d, "empty", &[]);
    let empty_group = bind(d, &empty, &[]);
    let inputs = layout(d, "motion rows", &[(CS, storage(true))]);
    let outputs = layout(
        d,
        "tick and samples",
        &[(CS, storage(true)), (CS, storage(false))],
    );
    let range = buffer(d, "snapshot tick", 16, U::STORAGE | U::COPY_DST);
    g.queue
        .write_buffer(&range, 0, &bytes_of(&[ticks, 0, 0, 0]));
    let mut samples = vec![];
    for step in &p.motion {
        let k = &p.kinds[step.kind];
        let m = shader(d, &p.modules[k.module], u32::MAX);
        let pipe = compute(
            d,
            "advance",
            &[&empty, &inputs, &empty, &outputs],
            &m,
            "advance",
            None,
        );
        let words = f32_bytes(&k.rows);
        let rows = buffer(
            d,
            "initial rows",
            words.len() as u64,
            U::STORAGE | U::COPY_DST,
        );
        g.queue.write_buffer(&rows, 0, &words);
        let inputs = bind(d, &inputs, &[rows.as_entire_binding()]);
        let width = step.fields.len() + step.checks.len();
        let len = u64::from(k.n_rows) * width as u64 * 16;
        if len == 0 {
            return Err("motion entry requires an emitted row and a write or check".into());
        }
        let out = buffer(
            d,
            "sampled writes and checks",
            len,
            U::STORAGE | U::COPY_SRC,
        );
        let read = buffer(d, "motion readback", len, U::MAP_READ | U::COPY_DST);
        let outputs = bind(
            d,
            &outputs,
            &[range.as_entire_binding(), out.as_entire_binding()],
        );
        let mut enc = d.create_command_encoder(&Default::default());
        {
            let mut cp = enc.begin_compute_pass(&Default::default());
            cp.set_pipeline(&pipe);
            for (i, group) in [&empty_group, &inputs, &empty_group, &outputs]
                .into_iter()
                .enumerate()
            {
                cp.set_bind_group(i as u32, group, &[]);
            }
            cp.dispatch_workgroups(k.n_rows.div_ceil(64), 1, 1);
        }
        enc.copy_buffer_to_buffer(&out, 0, &read, 0, Some(len));
        g.queue.submit([enc.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        read.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        d.poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let mapped = read.get_mapped_range(..).map_err(|e| e.to_string())?;
        let values: Vec<_> = mapped
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        let column = |j: usize| {
            values
                .chunks_exact(width * 4)
                .map(move |r| [r[4 * j], r[4 * j + 1]])
        };
        let fields = step
            .fields
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), column(i).map(|x| x[0]).collect()))
            .collect();
        let checks = step
            .checks
            .iter()
            .enumerate()
            .map(|(j, n)| (n.clone(), column(j + step.fields.len()).collect()))
            .collect();
        samples.push(MotionSample {
            kind: k.name.clone(),
            fields,
            checks,
        });
    }
    Ok(samples)
}
