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
        let packets =
            make(u64::from(capacity) * u64::from(self.count) * 16 * decl.kind.packet_lanes(), rw, "per-tick packets");
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
            events: BTreeMap::new(),
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
    events: BTreeMap<(u32, u32), BTreeMap<u64, [u32; 4]>>,
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
        self.events.clone_from(&other.events);
    }

    pub fn is_baked(&self) -> bool {
        self.baked
    }
    fn packet(&self, row: u32, tick: u32, bytes: &mut Vec<u8>) {
        let level = |default| self.changes.range((row, 0)..=(row, tick)).next_back().map_or(default, |(_, v)| *v);
        let mut append = |v: [u32; 4]| bytes.extend(v.into_iter().flat_map(u32::to_le_bytes));
        match &self.source {
            Source::Levels(v) => append(level(*v)),
            Source::Samples(samples) => append(level(samples[tick as usize])),
            Source::Events(slots) => {
                let events = self.events.get(&(row, tick));
                let count = events.map_or(0, BTreeMap::len);
                append([count as u32, 0, 0, 0]);
                if let Some(events) = events {
                    for value in events.values() {
                        append(*value);
                    }
                }
                bytes.resize(bytes.len() + 16 * (*slots as usize - count), 0);
            }
        }
    }

    /// Check all readers before mutating any log. A duplicate event is an idempotent delivery.
    fn accepts(&self, p: &Program, u: &Update) -> Result<(), Error> {
        let (row, k) = (u.row, u.tick);
        if k == 0 || k > p.spec.ticks || row >= p.count || u.value.iter().any(|v| !f32::from_bits(*v).is_finite()) {
            return Err(Error::Usage(
                format!("row {row}, tick {k}: use a declared row, tick 1..={} and finite components", p.spec.ticks)
                    .into(),
            ));
        }
        match (&self.source, u.sequence) {
            (Source::Events(slots), Some(seq)) => {
                if let Some(old) = self.seq.get(&(row, seq)) {
                    if *old != (k, u.value) {
                        return Err(Error::Usage(
                            "event sequence reused with different data: send a new sequence".into(),
                        ));
                    }
                } else if self.events.get(&(row, k)).is_some_and(|events| events.len() >= *slots as usize) {
                    return Err(Error::Usage(
                        "event capacity exceeded: declare more slots or send on a different tick".into(),
                    ));
                }
            }
            (Source::Events(_), None) => {
                return Err(Error::Usage("event input needs a sequence: use write_tick".into()));
            }
            (Source::Levels(_) | Source::Samples(_), Some(_)) => {
                return Err(Error::Usage("level input: omit the event sequence".into()));
            }
            (Source::Levels(_) | Source::Samples(_), None) => {}
        }
        Ok(())
    }
    fn apply(&mut self, u: &Update) {
        let k = u.tick;
        if let Some(seq) = u.sequence {
            if self.seq.insert((u.row, seq), (k, u.value)).is_some() {
                return;
            }
            self.events.entry((u.row, k)).or_default().insert(seq, u.value);
        } else {
            self.changes.insert((u.row, k), u.value);
        }
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
    }

    pub fn moment(p: &Program, time: f32) -> (u32, f32) {
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
                self.packet(r, k, &mut bytes);
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
                    self.packet(r, k, &mut bytes);
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

/// Validate the finite take and every reader up front, including legacy frame-uniform readers.
pub(super) struct Update {
    pub row: u32,
    pub tick: u32,
    pub sequence: Option<u64>,
    value: [u32; 4],
}
impl Update {
    pub fn new(row: u32, tick: u32, value: &[f32], sequence: Option<u64>) -> Self {
        let mut words = [0; 4];
        for (word, v) in words.iter_mut().zip(value) {
            *word = v.to_bits();
        }
        Self { row, tick, sequence, value: words }
    }
}
pub(super) fn write_tick(
    programs: &[Program],
    states: &mut [Timeline],
    live: &mut [super::live::Live],
    input: (usize, usize),
    value: &[f32],
    update: Update,
) -> Result<(), Error> {
    let (slot, frame_offset) = input;
    let readers = programs.iter().enumerate().filter(|(_, p)| p.spec.input as usize == slot);
    if live.is_empty() && readers.clone().next().is_none() {
        return Err(Error::Usage("input is frame-rate: use set_input".into()));
    }
    if !live.is_empty()
        && (update.row != 0
            || update.sequence.is_some()
            || update.tick == 0
            || value.iter().any(|v| !v.is_finite())
            || live.iter().any(|s| update.tick > s.count()))
    {
        return Err(Error::Usage(
            "shared-row level input: supply finite values for row zero at a valid tick, without an event sequence"
                .into(),
        ));
    }
    for (i, p) in readers.clone() {
        states[i].accepts(p, &update)?;
    }
    for (i, _) in readers {
        states[i].apply(&update);
    }
    for s in live {
        s.set(frame_offset, value, update.tick);
    }
    Ok(())
}
pub(super) fn set_level(
    programs: &[Program],
    states: &mut [Timeline],
    slot: usize,
    time: f32,
    value: &[f32],
) -> Result<(), Error> {
    let readers = programs.iter().enumerate().filter(|(_, p)| p.spec.input as usize == slot);
    for (i, p) in readers.clone() {
        let u = Update::new(0, Timeline::moment(p, time).0.max(1), value, None);
        states[i].accepts(p, &u)?;
    }
    for (i, p) in readers {
        let tick = Timeline::moment(p, time).0.max(1);
        for row in 0..p.count {
            states[i].apply(&Update::new(row, tick, value, None));
        }
    }
    Ok(())
}
