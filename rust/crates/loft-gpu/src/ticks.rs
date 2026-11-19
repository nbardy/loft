//! A row-local recurrence. The compiled module owns the types, and the instance owns all mutable rows.
//! The finite take keeps the input prefix on the host. A live dispatch uploads at most 64 consecutive packets.
use crate::{Buffer, storage};
use loft_ir::Error;
use loft_ir::bundle::{Bundle, Id, Init, InputKind, RowStep, Shape};
use loft_ir::identity::{TickKey, build};
use std::collections::BTreeMap;
use wgpu::util::DeviceExt;

pub(super) fn data(b: &Bundle, r: Id) -> &[u8] {
    let Init::Bytes(s) = b.resources[r as usize].init else { unreachable!("validated supplied rows") };
    &b.sections[s as usize]
}

pub(super) struct Program {
    advance: wgpu::ComputePipeline,
    show: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    plans: Buffer,
    pub spec: RowStep,
    pub count: u32,
    bytes: u64,
}

impl Program {
    pub fn new(
        d: &wgpu::Device,
        b: &Bundle,
        s: &RowStep,
        m: &wgpu::ShaderModule,
        cache: Option<&wgpu::PipelineCache>,
    ) -> Self {
        let mut entries = [storage(0, true), storage(1, true), storage(2, false), storage(3, true), storage(4, false)];
        entries[0].ty = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("row-local ticks"),
            entries: &entries,
        });
        let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry| {
            d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pl),
                module: m,
                entry_point: Some(entry),
                cache,
                compilation_options: Default::default(),
            })
        };
        let plans = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("plans"),
            contents: data(b, s.plan),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let Shape::Rows { rows: count, .. } = b.resources[s.seed as usize].shape else { unreachable!() };
        Self {
            advance: pipeline("advance"),
            show: pipeline("show"),
            layout,
            plans,
            spec: s.clone(),
            count,
            bytes: data(b, s.seed).len() as u64,
        }
    }

    pub fn start(&self, d: &wgpu::Device, q: &wgpu::Queue, b: &Bundle, output: &Buffer) -> Timeline {
        let decl = &b.inputs[self.spec.input as usize];
        let mut level = [0; 4];
        for (x, v) in level.iter_mut().zip(&decl.def) {
            *x = v.0;
        }
        let (source, keep, baked) = match &decl.kind {
            InputKind::Level => (Source::Levels(level), 60, false),
            InputKind::Events { slots } => (Source::Events(*slots), 60, false),
            InputKind::Script { samples } => (
                Source::Samples(
                    data(b, *samples)
                        .chunks_exact(16)
                        .map(|r| std::array::from_fn(|i| u32::from_le_bytes(r[i * 4..i * 4 + 4].try_into().unwrap())))
                        .collect(),
                ),
                1,
                true,
            ),
        };
        let make = |size, usage, label| {
            d.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
        };
        let rw = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
        let state = make(self.bytes * (2 + u64::from(self.spec.ticks / keep)), rw, "current and checkpoints");
        q.write_buffer(&state, 0, data(b, self.spec.seed));
        q.write_buffer(&state, self.bytes, data(b, self.spec.seed));
        let capacity = if baked { self.spec.ticks + 1 } else { 65 };
        let packets = make(u64::from(capacity) * u64::from(self.count) * 16, rw, "per-tick packets");
        let clock = make(32, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, "tick clock");
        let entries: Vec<_> = [&clock, &self.plans, &state, &packets, output]
            .into_iter()
            .enumerate()
            .map(|(i, b)| wgpu::BindGroupEntry { binding: i as u32, resource: b.as_entire_binding() })
            .collect();
        let group = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("row-local state"),
            layout: &self.layout,
            entries: &entries,
        });
        Timeline {
            group,
            state,
            clock,
            packets,
            source,
            changes: BTreeMap::new(),
            seq: BTreeMap::new(),
            keep,
            baked,
            done: 0,
            valid: 0,
            dirty: None,
            device: d.clone(),
        }
    }
}

#[derive(Clone)]
enum Source {
    Levels([u32; 4]),
    Samples(Vec<[u32; 4]>),
    Events(u32),
}

pub(super) struct Timeline {
    group: wgpu::BindGroup,
    state: Buffer,
    clock: Buffer,
    packets: Buffer,
    device: wgpu::Device,
    source: Source,
    changes: BTreeMap<(u32, u32), [u32; 4]>,
    seq: BTreeMap<(u32, u64), (u32, [u32; 4])>,
    keep: u32,
    baked: bool,
    pub done: u32,
    valid: u32,
    dirty: Option<u32>,
}

impl Timeline {
    pub fn copy_from(&mut self, other: &Self, enc: &mut wgpu::CommandEncoder) {
        enc.copy_buffer_to_buffer(&other.state, 0, &self.state, 0, None);
        self.baked = other.baked;
        self.done = other.done;
        self.valid = other.valid;
        self.dirty = other.dirty;
        self.changes.clone_from(&other.changes);
        self.seq.clone_from(&other.seq);
    }

    pub fn is_baked(&self) -> bool {
        self.baked
    }
    fn packet(&self, row: u32, k: u32) -> [u32; 4] {
        match &self.source {
            Source::Events(_) => self.changes.get(&(row, k)).copied().unwrap_or([0; 4]),
            source => self.changes.range((row, 0)..=(row, k)).next_back().map_or_else(
                || match source {
                    Source::Samples(a) => a[k as usize],
                    Source::Levels(v) => *v,
                    Source::Events(_) => unreachable!(),
                },
                |(_, v)| *v,
            ),
        }
    }

    pub fn write(&mut self, p: &Program, row: u32, k: u32, values: &[f32], sequence: Option<u64>) -> Result<(), Error> {
        if k == 0 || k > p.spec.ticks || row >= p.count || values.len() > 4 || values.iter().any(|x| !x.is_finite()) {
            return Err(Error::Usage(
                format!("row {row}, tick {k}: use a declared row, tick 1..={} and finite components", p.spec.ticks)
                    .into(),
            ));
        }
        let mut v = [0; 4];
        for (x, y) in v.iter_mut().zip(values) {
            *x = y.to_bits();
        }
        match (&self.source, sequence) {
            (Source::Events(slots), Some(seq)) => {
                if let Some(old) = self.seq.get(&(row, seq)) {
                    return if *old == (k, v) {
                        Ok(())
                    } else {
                        Err(Error::Usage("event sequence reused with different data: send a new sequence".into()))
                    };
                }
                if *slots != 1 || self.changes.contains_key(&(row, k)) {
                    return Err(Error::Usage(
                        "one event packet fits this tick: allocate a wider event packet or choose a different tick"
                            .into(),
                    ));
                }
                self.seq.insert((row, seq), (k, v));
            }
            (Source::Events(_), None) => {
                return Err(Error::Usage("event input needs a sequence: use write_tick".into()));
            }
            (Source::Levels(_) | Source::Samples(_), Some(_)) => {
                return Err(Error::Usage("level input: omit the event sequence".into()));
            }
            (Source::Levels(_) | Source::Samples(_), None) => {}
        }
        self.changes.insert((row, k), v);
        // show() reads a retained slot without writing slot zero. On the first live input the latter
        // still holds the end of the bake. Resume from a checkpoint even if no tick was presented yet.
        if self.baked {
            self.dirty = Some(k);
        }
        self.baked = false;
        self.valid = self.valid.min(k - 1);
        if self.done >= k {
            self.dirty = Some(self.dirty.map_or(k, |old| old.min(k)));
        }
        Ok(())
    }

    fn moment(p: &Program, time: f32) -> (u32, f32) {
        let rate = p.spec.hz as f32;
        let t = (time * rate).clamp(0.0, p.spec.ticks as f32);
        // The canonical f32 representation of tick/rate can multiply back to one ulp below tick.
        if time == t.round() / rate { (t.round() as u32, 0.0) } else { (t as u32, t.fract()) }
    }

    pub fn key(&self, p: &Program, time: f32) -> TickKey {
        let (tick, f) = Self::moment(p, time);
        let mut bytes = Vec::new();
        // Canonical packets, including scripts: changing the grouping or sending a redundant level retains identity.
        for k in 0..=tick {
            for r in 0..p.count {
                for x in self.packet(r, k) {
                    bytes.extend(x.to_le_bytes());
                }
            }
        }
        TickKey { checkpoint: tick / self.keep * self.keep, prefix: build(&bytes), tick, fraction: f.to_bits() }
    }

    // Encode the uploads in order. Queue writes to a reused uniform would give all dispatches in a catch-up
    // or two frames in one encoder the *last* clock and packet values.
    fn upload(&self, enc: &mut wgpu::CommandEncoder, buf: &Buffer, bytes: &[u8]) {
        let stage = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes,
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        enc.copy_buffer_to_buffer(&stage, 0, buf, 0, None);
    }

    fn run(
        &self,
        p: &Program,
        enc: &mut wgpu::CommandEncoder,
        from: u32,
        to: u32,
        slot: u32,
        f: f32,
        show: bool,
        ts: Option<&wgpu::QuerySet>,
    ) {
        if !show && from < to {
            let mut bytes = Vec::with_capacity(((to - from) * p.count * 16) as usize);
            for k in from + 1..=to {
                for r in 0..p.count {
                    for x in self.packet(r, k) {
                        bytes.extend(x.to_le_bytes());
                    }
                }
            }
            self.upload(enc, &self.packets, &bytes);
        }
        let words = [from, to, slot, self.keep, p.count, f.to_bits(), (1.0 / p.spec.hz as f32).to_bits(), from + 1];
        self.upload(enc, &self.clock, &words.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>());
        let timestamp_writes = ts.map(|query_set| wgpu::ComputePassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        });
        let mut pass = enc
            .begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("advance and present"), timestamp_writes });
        pass.set_pipeline(if show { &p.show } else { &p.advance });
        pass.set_bind_group(0, &self.group, &[]);
        pass.dispatch_workgroups(p.count.div_ceil(32), 1, 1);
    }

    pub fn bake(&mut self, p: &Program, enc: &mut wgpu::CommandEncoder, ts: Option<&wgpu::QuerySet>) {
        self.run(p, enc, 0, p.spec.ticks, 1, 0.0, false, ts);
        self.valid = p.spec.ticks;
    }

    pub fn encode(
        &mut self,
        p: &Program,
        enc: &mut wgpu::CommandEncoder,
        time: f32,
        ts: Option<&wgpu::QuerySet>,
    ) -> usize {
        let (to, f) = Self::moment(p, time);
        if self.baked {
            self.run(p, enc, to, to, 1 + to, f, true, ts);
            self.done = to;
            return 1;
        }
        let (mut from, mut slot) = if self.dirty.take().is_some()
            || to < self.done
            || to <= self.valid
            || (self.keep == 1 && self.valid > self.done)
        {
            let k = self.valid.min(to) / self.keep;
            (k * self.keep, 1 + k)
        } else {
            (self.done, 0)
        };
        let mut n = 0;
        loop {
            let end = to.min(from.saturating_add(64));
            self.run(p, enc, from, end, slot, f, false, ts.filter(|_| end == to));
            n += 1;
            if end == to {
                break;
            }
            from = end;
            slot = 0;
        }
        self.valid = self.valid.max(to);
        self.done = to;
        n
    }
    pub fn copy(
        &self,
        p: &Program,
        enc: &mut wgpu::CommandEncoder,
        tick: Option<u32>,
        dest: &Buffer,
        offset: u64,
    ) -> Result<(), Error> {
        let slot = match tick {
            None if self.baked => 1 + self.done,
            None => 0,
            Some(t) if t <= self.valid && t % self.keep == 0 => 1 + t / self.keep,
            Some(_) => {
                return Err(Error::Usage("state was not kept: present that tick first and copy current state".into()));
            }
        };
        enc.copy_buffer_to_buffer(&self.state, u64::from(slot) * p.bytes, dest, offset, Some(p.bytes));
        Ok(())
    }
}
