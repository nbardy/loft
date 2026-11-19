//! Dirty-kind pipeline selection and publication of one checked generation.
use super::*;
use loft_passes::tier0::{EditClass, TraceEdit, TraceSource};
use loft_runner::promotion::{self, Pending};
use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, TryRecvError},
    },
    time::Duration,
};

#[derive(Clone)]
struct Pipelines {
    module: wgpu::ShaderModule,
    render: wgpu::RenderPipeline,
    shade: wgpu::ComputePipeline,
}

impl Pipelines {
    fn current(kind: &KindGpu) -> Self {
        Self {
            module: kind.module.clone(),
            render: kind.render.clone(),
            shade: kind.shade.clone(),
        }
    }
}

struct PipelineCreates {
    foreground_id: std::thread::ThreadId,
    foreground: AtomicUsize,
    background: AtomicUsize,
}

impl PipelineCreates {
    fn new() -> Self {
        Self {
            foreground_id: std::thread::current().id(),
            foreground: 0.into(),
            background: 0.into(),
        }
    }

    fn record(&self) {
        let counter = if std::thread::current().id() == self.foreground_id {
            &self.foreground
        } else {
            &self.background
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

struct Job {
    generation: u64,
    source: String,
    done: Receiver<Result<Pipelines, String>>,
}

pub struct EditReceipt {
    pub class: EditClass,
    pub generation: u64,
    pub parse_check: Duration,
    pub bounds_lowering: Duration,
    pub upload: Duration,
    pub elapsed: Duration,
    pub foreground_creates: usize,
    pub dirtied_rows: Vec<u32>,
    pub tape_writes: Vec<Range<usize>>,
    pub caster_changed: bool,
}

pub struct TraceSession {
    pub loaded: Loaded,
    source: TraceSource,
    tape: TraceEdit,
    fallback: Pipelines,
    instances: Vec<loft_passes::program::Instance>,
    instance_slots: Vec<usize>,
    rows: Vec<f32>,
    stride: usize,
    shadow_size: u32,
    pending: Pending<Pipelines>,
    job: Option<Job>,
    creates: Arc<PipelineCreates>,
    edited: bool,
}

fn pass_kinds(passes: &[Pass], select: fn(&Pass) -> Option<&[u32]>) -> &[u32] {
    passes.iter().find_map(select).unwrap_or_default()
}

fn shadow_kinds(passes: &[Pass]) -> &[u32] {
    pass_kinds(passes, |p| {
        if let Pass::Shadow(k) = p {
            Some(k)
        } else {
            None
        }
    })
}

// Queue writes use aligned, contiguous ranges, including a single vec4 radius
// when the instruction sequence and remaining pool slots are unchanged.
fn changed_ranges(old: &[u8], new: &[u8]) -> Vec<Range<usize>> {
    let differs = |i: usize| old.get(i..i + 4) != new.get(i..i + 4);
    let mut ranges = Vec::new();
    let mut i = 0;
    while i < new.len() {
        if !differs(i) {
            i += 4;
            continue;
        }
        let start = i;
        while i < new.len() && differs(i) {
            i += 4;
        }
        ranges.push(start..i);
    }
    ranges
}

impl TraceSession {
    pub fn new(
        g: &Gpu,
        p: &Program,
        source: TraceSource,
        tape: TraceEdit,
        size: (u32, u32),
    ) -> Result<Self, String> {
        let kind = source.kind();
        let loaded = Loaded::load(g, p, size, None, Some((kind, &tape)), Aov::Beauty)?;
        let fallback = Pipelines::current(&loaded.kinds[kind]);
        let mut instance_slots = vec![usize::MAX; p.kinds[kind].n_rows as usize];
        for (i, inst) in p.instances.iter().enumerate() {
            if inst.kind as usize == kind {
                instance_slots[inst.row as usize] = i;
            }
        }
        if instance_slots.contains(&usize::MAX) {
            return Err("edit session could not find a culling slot for every row".into());
        }
        Ok(Self {
            loaded,
            source,
            tape,
            fallback,
            instances: p.instances.clone(),
            instance_slots,
            rows: p.kinds[kind].rows.clone(),
            stride: p.kinds[kind].stride as usize,
            shadow_size: p.shadow_size,
            pending: Pending::default(),
            job: None,
            creates: Arc::new(PipelineCreates::new()),
            edited: false,
        })
    }

    pub fn describe(&self) -> String {
        self.tape.describe()
    }

    pub fn edit(
        &mut self,
        g: &Gpu,
        src: &loft_ir::Sources,
        overrides: &[(String, f64)],
        start: Instant,
    ) -> Result<EditReceipt, String> {
        let before_creates = self.creates.foreground.load(Ordering::Relaxed);
        let next = self
            .source
            .prepare(src, overrides)
            .map_err(|d| d.render(src))?;
        let class = next.class(&self.tape);
        match class {
            EditClass::ConstantPool | EditClass::StructuralTape => {}
            EditClass::IncompatibleLayout => {
                return Err("incompatible trace tape layout, reload the scene".into());
            }
        }
        let kind = self.source.kind();
        let tape = next.tape.bytes();
        let tape_writes = changed_ranges(&self.tape.bytes(), &tape);
        let dirty_columns = self.stride - abi::HIDDEN_WORDS as usize;
        let mut row_patches = Vec::new();
        for (i, ([lo, hi], lip)) in next.local_bounds.iter().zip(&next.lips).enumerate() {
            let begin = i * self.stride + dirty_columns;
            let words = [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], *lip];
            if self.rows[begin..begin + words.len()]
                .iter()
                .map(|w| w.to_bits())
                .ne(words.iter().map(|w| w.to_bits()))
            {
                row_patches.push((i, begin, words));
            }
        }
        let mut instance_patches = Vec::new();
        for (&slot, &next) in self.instance_slots.iter().zip(&next.bounds) {
            let old = self.instances[slot];
            if old.lo != next.lo || old.hi != next.hi {
                instance_patches.push((slot, next));
            }
        }
        let casters = shadow_kinds(&self.loaded.passes);
        let caster_changed = !instance_patches.is_empty() && casters.contains(&(kind as u32));
        for &(slot, bound) in &instance_patches {
            self.instances[slot] = bound;
        }
        if caster_changed && let Some(shadow) = &mut self.loaded.shadow {
            let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
            for inst in self.instances.iter().filter(|i| casters.contains(&i.kind)) {
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(inst.lo[axis]);
                    hi[axis] = hi[axis].max(inst.hi[axis]);
                }
            }
            (shadow.view_proj, shadow.texel, shadow.dir) =
                sun_camera(self.loaded.sun, lo, hi, self.shadow_size);
        }
        if self.loaded.kinds[kind].shade != self.fallback.shade {
            self.install(&g.device, self.fallback.clone());
        }

        // Loaded is exclusively borrowed through publication. Render commands
        // for this session cannot observe the middle of these queue writes.
        let write_start = Instant::now();
        let dst = &self.loaded.kinds[kind].rows;
        for &(_, offset, words) in &row_patches {
            self.rows[offset..offset + words.len()].copy_from_slice(&words);
            g.queue
                .write_buffer(dst, (offset * 4) as u64, &f32_bytes(&words));
        }
        for range in &tape_writes {
            g.queue.write_buffer(
                dst,
                (self.rows.len() * 4 + range.start) as u64,
                &tape[range.clone()],
            );
        }
        for (slot, bound) in instance_patches {
            let mut bytes = [0; 32];
            bytes[..12].copy_from_slice(&f32_bytes(&bound.lo));
            bytes[12..16].copy_from_slice(&bound.kind.to_le_bytes());
            bytes[16..28].copy_from_slice(&f32_bytes(&bound.hi));
            bytes[28..].copy_from_slice(&bound.row.to_le_bytes());
            g.queue.write_buffer(
                &self.loaded.instance_bounds,
                (slot * bytes.len()) as u64,
                &bytes,
            );
        }
        let finished = Instant::now();
        self.tape = next.tape;
        self.pending.request(next.shader);
        self.edited = true;
        Ok(EditReceipt {
            class,
            generation: self.pending.generation(),
            parse_check: next.parse_check,
            bounds_lowering: next.bounds_lowering,
            upload: finished - write_start,
            elapsed: finished - start,
            foreground_creates: self.creates.foreground.load(Ordering::Relaxed) - before_creates,
            dirtied_rows: row_patches.iter().map(|(row, ..)| *row as u32).collect(),
            tape_writes,
            caster_changed,
        })
    }

    fn install(&mut self, d: &wgpu::Device, pipelines: Pipelines) {
        let kind = &mut self.loaded.kinds[self.source.kind()];
        kind.module = pipelines.module;
        kind.render = pipelines.render;
        kind.shade = pipelines.shade;
        let l = &mut self.loaded;
        let kinds = pass_kinds(
            &l.passes,
            |p| if let Pass::Vis(k) = p { Some(k) } else { None },
        );
        l.bundle = draw_bundle(d, &l.g0, &l.kinds, kinds, &l.draw_args, None);
        if let Some(shadow) = &mut l.shadow {
            shadow.bundle = draw_bundle(
                d,
                &shadow.groups.0,
                &l.kinds,
                shadow_kinds(&l.passes),
                &shadow.args,
                Some(&shadow.groups.1),
            );
        }
        l.prepare_passes();
    }

    /// A preparation failure leaves the last validated tape installed.
    pub fn poll_promotion(&mut self, g: &Gpu) -> Result<bool, String> {
        if let Some(job) = &self.job {
            let result = match job.done.try_recv() {
                Err(TryRecvError::Empty) => return Ok(false),
                Err(TryRecvError::Disconnected) => Err("trace pipeline worker stopped".into()),
                Ok(result) => result,
            };
            let job = self.job.take().expect("completed trace job");
            if let Some(pipeline) = self.pending.complete(job.generation, job.source, result)? {
                self.install(&g.device, pipeline);
                self.edited = false;
                return Ok(true);
            }
        }
        let Some((generation, source)) = self.pending.ready() else {
            return Ok(!self.edited);
        };
        if let Some(pipelines) = self.pending.cached(generation, &source) {
            self.install(&g.device, pipelines);
            self.edited = false;
            return Ok(true);
        }
        let d = g.device.clone();
        let [g0, g1, g2] = [
            &self.loaded.bgl0,
            &self.loaded.bgl_kind,
            &self.loaded.bgl_shade,
        ]
        .map(Clone::clone);
        let wgsl = source.clone();
        let creates = self.creates.clone();
        let done = promotion::worker(move || {
            let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
            let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("promoted trace"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&wgsl)),
            });
            creates.record();
            let render = render(
                &d,
                "promoted visibility",
                &[&g0, &g1],
                &module,
                ("vs", "fs"),
                VIS,
                true,
                Some(wgpu::Face::Front),
                None,
            );
            creates.record();
            let shade = compute(
                &d,
                "promoted shade",
                &[&g0, &g1, &g2],
                &module,
                "shade",
                None,
            );
            pollster::block_on(scope.pop()).map_or(
                Ok(Pipelines {
                    module,
                    render,
                    shade,
                }),
                |e| Err(e.to_string()),
            )
        })
        .map_err(|e| e.to_string())?;
        self.job = Some(Job {
            generation,
            source,
            done,
        });
        Ok(false)
    }

    pub fn measure_paired(
        &mut self,
        g: &Gpu,
        t: f32,
        count: usize,
    ) -> Result<Vec<(f64, Vec<f64>)>, String> {
        let current = Pipelines::current(&self.loaded.kinds[self.source.kind()]);
        let mut observations = Vec::with_capacity(count);
        let result = (|| {
            for i in 0..count {
                let mut pair = [None, None];
                for tier in [i % 2, 1 - i % 2] {
                    let target = if tier == 0 { &self.fallback } else { &current };
                    self.install(&g.device, target.clone());
                    self.loaded.frame(g, t);
                    pair[tier] = self.loaded.gpu_ms(g)?;
                }
                if let [Some((edit, edit_passes)), Some((promoted, promoted_passes))] = pair {
                    observations.push((
                        edit - promoted,
                        edit_passes
                            .iter()
                            .zip(promoted_passes)
                            .map(|(a, b)| a - b)
                            .collect(),
                    ));
                }
            }
            Ok(observations)
        })();
        self.install(&g.device, current);
        result
    }

    pub fn clean_pipelines(&self) -> Vec<(wgpu::RenderPipeline, wgpu::ComputePipeline)> {
        self.loaded
            .kinds
            .iter()
            .enumerate()
            .filter(|(kind, _)| *kind != self.source.kind())
            .map(|(_, kind)| (kind.render.clone(), kind.shade.clone()))
            .collect()
    }

    pub fn kind(&self) -> usize {
        self.source.kind()
    }

    pub fn promotion_stats(&self) -> (usize, u64) {
        (
            self.creates.background.load(Ordering::Relaxed),
            self.pending.stale(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_edit_keeps_bounds_and_pixels_across_evaluators() {
        use loft_ir::Sources;
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut inputs: Vec<_> = std::fs::read_dir(repo.join("lib"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "loft"))
            .collect();
        inputs.sort();
        let mut base = Sources::default();
        for input in inputs {
            base.add(
                input.display().to_string(),
                std::fs::read_to_string(&input).unwrap(),
            );
        }
        let scene = include_str!("../../tests/d1_edit/literal-old.loft");
        let mut edited = Sources {
            files: base.files.clone(),
        };
        base.add("base.loft", scene);
        edited.add(
            "general.loft",
            scene.replace(
                "sphere(p - shift, 0.59)",
                "max(sphere(p - shift, 0.64), p.x - 0.35)",
            ),
        );

        let (before, _, compiled) = loft_passes::compile(&base, &[]).unwrap();
        let (after, _, _) = loft_passes::compile(&edited, &[]).unwrap();
        let (source, tape) = TraceSource::new(base, compiled, &before, "EditSphere").unwrap();
        let prepared = source.prepare(&edited, &[]).unwrap();
        assert!(prepared.tape.describe().starts_with("Pair(Max,"));
        let kind = source.kind();
        let rows = &after.kinds[kind].rows;
        let hidden = after.kinds[kind].stride as usize - abi::HIDDEN_WORDS as usize;
        let [lo, hi] = prepared.local_bounds[0];
        assert_eq!(
            &rows[hidden..hidden + 7],
            &[lo, hi]
                .concat()
                .into_iter()
                .chain(prepared.lips)
                .collect::<Vec<_>>()
        );

        let Ok(gpu) = gpu() else { return };
        let mut session = TraceSession::new(&gpu, &before, source, tape, (1280, 800)).unwrap();
        session.edit(&gpu, &edited, &[], Instant::now()).unwrap();
        let limit = Instant::now() + Duration::from_secs(30);
        while !session.poll_promotion(&gpu).unwrap() {
            assert!(Instant::now() < limit, "promotion timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        session.loaded.frame(&gpu, 0.0);
        let pixels = session.loaded.pixels(&gpu).unwrap();
        let promoted = Pipelines::current(&session.loaded.kinds[kind]);
        for (mode, name) in [(16u32, "fused"), (3, "compact VM"), (0, "full VM")] {
            // Alter only the selector. All three paths see the same tape,
            // row data, pipelines, command counts, and timestamps.
            gpu.queue.write_buffer(
                &session.loaded.kinds[kind].rows,
                (session.rows.len() * 4 + 4) as u64,
                &mode.to_le_bytes(),
            );
            session.install(&gpu.device, session.fallback.clone());
            session.loaded.frame(&gpu, 0.0);
            let actual = session.loaded.pixels(&gpu).unwrap();
            assert_eq!(actual.len(), pixels.len());
            assert!(
                actual.iter().eq(&pixels),
                "{name} differs at {} RGBA bytes",
                actual.iter().zip(&pixels).filter(|(a, b)| a != b).count()
            );

            session.install(&gpu.device, promoted.clone());
            let observations = session.measure_paired(&gpu, 0.0, 132).unwrap();
            if let Some((_, passes)) = observations.first() {
                let stats = |select: &dyn Fn(&(f64, Vec<f64>)) -> f64| {
                    let mut samples: Vec<_> = observations[12..].iter().map(select).collect();
                    samples.sort_by(f64::total_cmp);
                    [samples[59], samples[113]]
                };
                let pass: Vec<_> = (0..passes.len()).map(|i| stats(&|(_, p)| p[i])).collect();
                println!(
                    "{name} paired frame p50/p95 {:?} ms, passes {pass:?}",
                    stats(&|(t, _)| *t)
                );
            }
        }
    }
}
