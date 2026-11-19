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
    let params_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("snapshot parameters"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 2, visibility: CS, ty: storage(true), count: None,
        }],
    });
    let params = buffer(d, "parameters", (p.params.len() as u64 * 4).max(16), U::STORAGE | U::COPY_DST);
    if !p.params.is_empty() { g.queue.write_buffer(&params, 0, &f32_bytes(&p.params)); }
    let parameters = d.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("snapshot parameters"), layout: &params_layout,
        entries: &[wgpu::BindGroupEntry { binding: 2, resource: params.as_entire_binding() }],
    });
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
    for step in &p.dispatches {
        let (name, n_rows, _, words) = step.input.view(&p.kinds).ok_or("dispatch input table is missing")?;
        let m = shader(d, &p.modules[step.module], u32::MAX);
        let pipe = compute(
            d,
            &step.entry,
            &[&params_layout, &inputs, &empty, &outputs],
            &m,
            &step.entry,
            None,
        );
        let words = f32_bytes(words);
        let rows = buffer(
            d,
            "initial rows",
            words.len() as u64,
            U::STORAGE | U::COPY_DST,
        );
        g.queue.write_buffer(&rows, 0, &words);
        let inputs = bind(d, &inputs, &[rows.as_entire_binding()]);
        let width = step.fields.len() + step.checks.len();
        let len = u64::from(n_rows) * width as u64 * 16;
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
            for (i, group) in [&parameters, &inputs, &empty_group, &outputs]
                .into_iter()
                .enumerate()
            {
                cp.set_bind_group(i as u32, group, &[]);
            }
            cp.dispatch_workgroups(n_rows.div_ceil(step.workgroup_x), 1, 1);
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
            kind: name.into(),
            fields,
            checks,
        });
    }
    Ok(samples)
}
