//! Existing, potentially cross-row Steps kernels keep one dispatch per tick. Each instance owns its
//! ping-pong pair, checkpoint table and time-stamped changes to the frame values those ticks read.
use super::*;
use std::{cell::Cell, collections::BTreeMap};
use wgpu::util::DeviceExt;

pub(super) struct Live {
    pub index: usize,
    layout: Stepping,
    pair: [Buffer; 2],
    groups: [(BindGroup, BindGroup); 2],
    checkpoints: Buffer,
    clocks: Buffer,
    frame: BindGroup,
    device: wgpu::Device,
    changes: BTreeMap<(usize, u32), f32>,
    initial: Vec<f32>,
    params: usize,
    done: Cell<u32>,
    valid: Cell<u32>,
    dirty: Cell<bool>,
}
impl Live {
    pub fn new<'a>(
        l: &Loft,
        d: &wgpu::Device,
        index: usize,
        pack: &dyn Fn(usize) -> &'a Buffer,
        enc: &mut wgpu::CommandEncoder,
        parent: Option<&Live>,
    ) -> Self {
        let st = &l.steppers[index];
        let bytes = state_bytes(&l.b.resources[st.res].shape);
        let make = |size, usage| {
            d.create_buffer(&wgpu::BufferDescriptor {
                label: Some("live steps"),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let x = pack(st.pack);
        let pair = [x.clone(), make(bytes, x.usage())];
        let checkpoints =
            make(bytes * (1 + u64::from(st.s.count / 60)), wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST);
        let clocks = make(60 * l.stride(), wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let frame = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &l.bgl0,
            entries: &group0(&clocks, l.frame_size, &l.detail, &l.scene),
        });
        let groups = std::array::from_fn(|parity| {
            let read = |p| if p == st.pack { &pair[1 - parity] } else { pack(p) };
            let write = |_| pair[parity].as_entire_binding();
            (
                pack_group(d, &l.bgl1.compute, l.packs.len(), &read),
                group(d, &l.bgl2[st.kernel], &l.b.kernels[st.kernel].writes, &write),
            )
        });
        let (initial, changes, done, valid, dirty) = match parent {
            None => {
                enc.copy_buffer_to_buffer(x, 0, &checkpoints, 0, Some(bytes));
                ([&l.params[..], &l.inputs[..]].concat(), BTreeMap::new(), 0, 0, false)
            }
            Some(p) => {
                enc.copy_buffer_to_buffer(&p.pair[1], 0, &pair[1], 0, None);
                enc.copy_buffer_to_buffer(&p.checkpoints, 0, &checkpoints, 0, None);
                (p.initial.clone(), p.changes.clone(), p.done.get(), p.valid.get(), p.dirty.get())
            }
        };
        Self {
            index,
            layout: st.s,
            pair,
            groups,
            checkpoints,
            clocks,
            frame,
            device: d.clone(),
            changes,
            initial,
            params: l.params.len(),
            done: Cell::new(done),
            valid: Cell::new(valid),
            dirty: Cell::new(dirty),
        }
    }
    pub fn set(&mut self, start: usize, values: &[f32], tick: u32) {
        for (i, x) in values.iter().enumerate() {
            self.changes.insert((start + i, tick), *x);
        }
        self.valid.set(self.valid.get().min(tick.saturating_sub(1)));
        if tick <= self.done.get() {
            self.dirty.set(true);
        }
    }
    pub fn set_at_time(&mut self, start: usize, values: &[f32], time: f32) {
        self.set(start, values, self.layout.at(time).max(1));
    }
    pub fn count(&self) -> u32 {
        self.layout.count
    }
    fn values(&self, k: u32) -> Vec<f32> {
        self.initial
            .iter()
            .enumerate()
            .map(|(i, v)| self.changes.range((i, 0)..=(i, k)).next_back().map_or(*v, |(_, x)| *x))
            .collect()
    }
    pub fn key(&self, t: f32) -> loft_ir::identity::TickKey {
        let tick = self.layout.at(t);
        let mut bytes = Vec::new();
        for k in 1..=tick {
            bytes.extend(floats(&self.values(k)));
        }
        loft_ir::identity::TickKey {
            tick,
            checkpoint: tick / 60 * 60,
            fraction: ((t - self.layout.from) / self.layout.dt).fract().to_bits(),
            prefix: loft_ir::identity::build(&bytes),
        }
    }
    pub fn encode(&self, l: &Loft, enc: &mut wgpu::CommandEncoder, time: f32) {
        let st = &l.steppers[self.index];
        let to = self.layout.at(time);
        let bytes = self.pair[1].size();
        let mut k = self.done.get();
        if self.dirty.replace(false) || to < k {
            k = to.min(self.valid.get()) / 60 * 60;
            enc.copy_buffer_to_buffer(&self.checkpoints, u64::from(k / 60) * bytes, &self.pair[0], 0, Some(bytes));
        }
        let stride = l.stride();
        while k < to {
            let end = to.min((k / 60 + 1) * 60);
            let mut upload = vec![0; ((end - k) as u64 * stride) as usize];
            for i in k + 1..=end {
                let vals = self.values(i);
                let (params, inputs) = vals.split_at(self.params);
                let data =
                    frame_bytes(&l.host_camera(), l.size, self.layout.from + i as f32 * self.layout.dt, params, inputs);
                let at = ((i - k - 1) as u64 * stride) as usize;
                upload[at..at + data.len()].copy_from_slice(&data);
            }
            let stage = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &upload,
                usage: wgpu::BufferUsages::COPY_SRC,
            });
            enc.copy_buffer_to_buffer(&stage, 0, &self.clocks, 0, None);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("shared-row ticks"),
                timestamp_writes: None,
            });
            for i in k + 1..=end {
                let (g1, g2) = &self.groups[(i % 2) as usize];
                l.dispatch(&mut pass, st.kernel, (&self.frame, ((i - k - 1) as u64 * stride) as u32), (g1, g2));
            }
            drop(pass);
            k = end;
            if k.is_multiple_of(60) {
                enc.copy_buffer_to_buffer(&self.pair[0], 0, &self.checkpoints, u64::from(k / 60) * bytes, Some(bytes));
            }
        }
        if to % 2 == 1 {
            enc.copy_buffer_to_buffer(&self.pair[1], 0, &self.pair[0], 0, Some(bytes));
        }
        self.done.set(to);
        self.valid.set(self.valid.get().max(to));
    }
}
