//! Author compute objects and build, load, frame and step dispatches.
use super::*;
use crate::artefact::{
    CompiledCompute, CompiledPass, ComputeResource, ComputeShape, ComputeSteps, ComputeWhen, Extent,
};

pub(super) struct ComputeRuns<'a> {
    baked: Option<&'a Path>,
    data: HashMap<&'a str, Resource>,
    pub build: Build,
    pub load: Vec<ComputeRun>,
    pub frame: Vec<RawDispatch>,
}

impl<'a> ComputeRuns<'a> {
    pub fn new(baked: &'a Path, compute: Option<&CompiledCompute>) -> Result<Self, Error> {
        let mut is_baked = false;
        if compute.is_some_and(|c| c.passes.iter().any(|p| matches!(p.when, ComputeWhen::Build))) {
            match std::fs::read_dir(baked) {
                Ok(entries) => {
                    for entry in entries {
                        let entry = entry.map_err(|e| Error::Input(format!("{}: {e}", baked.display())))?;
                        let name = entry.file_name();
                        if name.to_str().is_some_and(|s| s.starts_with("build.") && s.ends_with(".bin")) {
                            is_baked = true;
                            break;
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(Error::Input(format!("{}: {e}", baked.display()))),
            }
        }
        // An empty build map means the first lower call, which carries the dispatches for the GPU owner.
        Ok(Self {
            baked: is_baked.then_some(baked),
            data: HashMap::new(),
            build: Build::default(),
            load: Vec::new(),
            frame: Vec::new(),
        })
    }

    pub fn resource(&self, key: &str) -> Result<Resource, Error> {
        self.data.get(key).copied().ok_or_else(|| Error::Input(format!("compute \"{key}\": missing resource")))
    }
}

fn compute_history_size(size: Extent, states: usize) -> Extent {
    match size {
        Extent::D2([w, h]) => Extent::D2([w, h * states]),
        Extent::D3([w, h, d]) => Extent::D3([w, h, d * states]),
    }
}

fn raw_texture(
    r: &ComputeResource,
    size: Extent,
    format: &'static str,
    usage: &'static [&'static str],
    init: Option<String>,
) -> TextureDesc {
    TextureDesc { label: r.key.clone(), size: None, extent: Some(size), format, sample_count: 1, usage, init }
}

pub(super) fn raw_layout(
    r: &ComputeResource,
    binding: usize,
    visibility: &'static [Stage],
    write: bool,
) -> LayoutEntry {
    match r.shape {
        ComputeShape::Buffer { .. } => {
            if write {
                buffer_layout(binding, visibility, BufferType::Storage)
            } else {
                storage(binding, visibility)
            }
        }
        ComputeShape::Texture { size, format } => {
            let view_dimension = match size {
                Extent::D2(_) => "2d",
                Extent::D3(_) => "3d",
            };
            let resource = if write {
                LayoutResource::StorageTexture { access: "write-only", format: format.name(), view_dimension }
            } else {
                LayoutResource::Texture {
                    sample_type: SampleType::UnfilterableFloat,
                    view_dimension,
                    multisampled: false,
                }
            };
            LayoutEntry { binding, visibility, resource }
        }
    }
}

struct ComputeGroup {
    group: Id,
    layout: Id,
}

struct ComputePassObjects {
    label: String,
    l0_group: Id,
    l1: Id,
    layout: Id,
    module: Id,
    pipeline: Id,
    inputs: Vec<Resource>,
    outputs: Vec<Resource>,
}

impl<'a> Lower<'a> {
    pub(super) fn raw_buffers(&mut self, compute: &mut ComputeRuns<'a>) -> Result<(), Error> {
        let Some(compiled) = &self.a.compute else { return Ok(()) };
        for p in &compiled.passes {
            for (_, r) in &p.writes {
                let build = matches!(p.when, ComputeWhen::Build);
                let init = match (build, compute.baked) {
                    (true, Some(baked)) => Some(self.baked_key(baked, r)?),
                    (false, _) | (true, None) => None,
                };
                let states = match &r.steps {
                    Some(ComputeWhen::Steps(s)) if s.keep > 0 => Some(1 + s.count.div_ceil(s.keep)),
                    Some(_) => {
                        return Err(Error::Input(format!("compute \"{}\": expected steps with positive keep", r.key)));
                    }
                    None => None,
                };
                let resource = match r.shape {
                    ComputeShape::Buffer { .. } => {
                        let usage = if build && init.is_none() { &["STORAGE", "COPY_SRC"][..] } else { STORAGE };
                        Resource::Buffer(self.buffer(&r.key, r.bytes * states.unwrap_or(1), usage, init))
                    }
                    ComputeShape::Texture { size, format } => {
                        let usage: &[&str] = if init.is_some() || states.is_some() {
                            &["TEXTURE_BINDING", "COPY_DST"][..]
                        } else {
                            match p.when {
                                ComputeWhen::Build => &["TEXTURE_BINDING", "STORAGE_BINDING", "COPY_SRC"],
                                ComputeWhen::Frame => &["TEXTURE_BINDING", "STORAGE_BINDING", "COPY_DST"],
                                ComputeWhen::Load | ComputeWhen::Steps(_) => &["TEXTURE_BINDING", "STORAGE_BINDING"],
                            }
                        };
                        let size = states.map_or(size, |n| compute_history_size(size, n));
                        Resource::Texture(add(&mut self.textures, raw_texture(r, size, format.name(), usage, init)))
                    }
                };
                compute.data.insert(&r.key, resource);
            }
        }
        Ok(())
    }

    fn baked_key(&mut self, baked: &Path, r: &ComputeResource) -> Result<String, Error> {
        let bytes = read_baked(baked, "build", &r.key)?;
        if bytes.len() != r.bytes {
            return Err(Error::Internal(format!("compute \"{}\": missing or wrong-sized bake data", r.key)));
        }
        Ok(self.blob(format!("compute.{}", r.key), bytes))
    }

    pub(super) fn author_compute(
        &mut self,
        scene: &SceneObjects<'_>,
        compute: &mut ComputeRuns<'_>,
    ) -> Result<(), Error> {
        let Some(compiled) = &self.a.compute else { return Ok(()) };
        let mut empty_compute = None;
        let mut frame_compute = None;
        for p in &compiled.passes {
            if matches!(p.when, ComputeWhen::Build) && compute.baked.is_some() {
                continue;
            }
            let writes = p.writes.iter().map(|(_, r)| (r, true));
            let reads = p.reads.iter().map(|(_, r)| (r, false));
            let previous = matches!(p.when, ComputeWhen::Steps(_))
                .then_some(&p.writes[..])
                .into_iter()
                .flatten()
                .map(|(_, r)| (r, false));
            let l1 = self.bgl(
                &p.name,
                writes
                    .chain(previous)
                    .chain(reads)
                    .enumerate()
                    .map(|(binding, (r, write))| raw_layout(r, binding, COMPUTE, write))
                    .collect(),
            );
            let l0 = if matches!(p.when, ComputeWhen::Frame) {
                frame_compute.get_or_insert_with(|| {
                    let label = "compute frame values";
                    let layout = self.bgl(label, vec![storage(0, COMPUTE), storage(1, COMPUTE)]);
                    let group = self.group(
                        label,
                        layout,
                        vec![
                            BindGroupEntry { binding: 0, resource: Resource::Buffer(scene.prelude.input) },
                            BindGroupEntry { binding: 1, resource: Resource::Buffer(scene.prelude.param) },
                        ],
                    );
                    ComputeGroup { layout, group }
                })
            } else {
                empty_compute.get_or_insert_with(|| {
                    let label = "compute no frame values";
                    let layout = self.bgl(label, vec![]);
                    let group = self.group(label, layout, vec![]);
                    ComputeGroup { layout, group }
                })
            };
            let label = format!("compute \"{}\"", p.name);
            let chunk = self.chunk(&p.wgsl)?;
            let module = self.module(&label, vec![chunk]);
            let layout = self.pipeline_layout(&label, vec![l0.layout, l1]);
            let pipeline = self.compute_pipeline(format!("{label} {}", p.entry), layout, module, &p.entry);
            let get = |(_, r): &(String, ComputeResource)| compute.resource(&r.key);
            let inputs = p.reads.iter().map(get).collect::<Result<_, _>>()?;
            let outputs = p.writes.iter().map(get).collect::<Result<_, _>>()?;
            let c = ComputePassObjects { label, l0_group: l0.group, l1, layout, module, pipeline, inputs, outputs };
            match &p.when {
                ComputeWhen::Build => self.compute_build(p, &c, &mut compute.build)?,
                ComputeWhen::Load => compute.load.push(ComputeRun::Once(RawDispatch {
                    run: self.make_dispatch(p, &c, pipeline, &c.outputs),
                    clear_buffers: None,
                    clear_textures: None,
                })),
                ComputeWhen::Frame => compute.frame.push(self.compute_frame(p, &c)?),
                ComputeWhen::Steps(steps) => {
                    compute.load.push(ComputeRun::Steps(Box::new(self.compute_steps(p, &c, steps)?)))
                }
            }
        }
        Ok(())
    }

    fn make_dispatch(
        &mut self,
        p: &CompiledPass,
        c: &ComputePassObjects,
        pipeline: Id,
        bindings: &[Resource],
    ) -> Dispatch {
        let entries = bindings
            .iter()
            .chain(&c.inputs)
            .copied()
            .enumerate()
            .map(|(binding, resource)| BindGroupEntry { binding, resource })
            .collect();
        let group = self.group(&c.label, c.l1, entries);
        Dispatch {
            label: c.label.clone(),
            pipeline,
            bind_groups: vec![c.l0_group, group],
            workgroups: Workgroups::Xyz(p.dispatch),
        }
    }

    fn compute_build(&mut self, p: &CompiledPass, c: &ComputePassObjects, build: &mut Build) -> Result<(), Error> {
        build.runs.push(RawDispatch {
            run: self.make_dispatch(p, c, c.pipeline, &c.outputs),
            clear_buffers: None,
            clear_textures: None,
        });
        for ((_, r), &out) in p.writes.iter().zip(&c.outputs) {
            let resource = match out {
                Resource::Buffer(id) => BuildResource::Buffer { id, bytes: r.bytes },
                Resource::Texture(id) => BuildResource::Texture { id },
                Resource::Sampler(_) => {
                    return Err(Error::Internal(format!("compute \"{}\": unexpected sampler output", r.key)));
                }
            };
            build.outputs.push(BuildOutput { name: r.key.clone(), resource });
        }
        Ok(())
    }

    fn compute_frame(&mut self, p: &CompiledPass, c: &ComputePassObjects) -> Result<RawDispatch, Error> {
        let mut clear_buffers = Vec::new();
        let mut clear_textures = Vec::new();
        for ((_, r), &out) in p.writes.iter().zip(&c.outputs) {
            match (&r.shape, out) {
                (ComputeShape::Buffer { .. }, Resource::Buffer(id)) => clear_buffers.push(id),
                (ComputeShape::Texture { size, format }, Resource::Texture(to)) => {
                    let from = self.zero_texture(r, *size, format.name());
                    clear_textures.push(TextureCopy { from, to });
                }
                (ComputeShape::Buffer { .. }, Resource::Texture(_) | Resource::Sampler(_))
                | (ComputeShape::Texture { .. }, Resource::Buffer(_) | Resource::Sampler(_)) => {
                    return Err(Error::Input(format!("compute \"{}\": mismatched resource", r.key)));
                }
            }
        }
        Ok(RawDispatch {
            run: self.make_dispatch(p, c, c.pipeline, &c.outputs),
            clear_buffers: Some(clear_buffers),
            clear_textures: Some(clear_textures),
        })
    }

    fn zero_texture(&mut self, r: &ComputeResource, size: Extent, format: &'static str) -> Id {
        let texture =
            TextureDesc { label: format!("{} zero", r.key), ..raw_texture(r, size, format, &["COPY_SRC"], None) };
        add(&mut self.textures, texture)
    }

    fn compute_steps(
        &mut self,
        p: &CompiledPass,
        c: &ComputePassObjects,
        steps: &ComputeSteps,
    ) -> Result<StepRuns, Error> {
        let mut state = Vec::new();
        for ((_, r), &out) in p.writes.iter().zip(&c.outputs) {
            state.push(match (&r.shape, out) {
                (ComputeShape::Buffer { .. }, Resource::Buffer(to)) => {
                    const USAGE: &[&str] = &["STORAGE", "COPY_SRC", "COPY_DST"];
                    let a = self.buffer(format!("{} A", r.key), r.bytes, USAGE, None);
                    let b = self.buffer(format!("{} B", r.key), r.bytes, USAGE, None);
                    StepState::Buffer { a, b, to, bytes: r.bytes }
                }
                (ComputeShape::Texture { size, format }, Resource::Texture(to)) => {
                    const USAGE: &[&str] = &["STORAGE_BINDING", "TEXTURE_BINDING", "COPY_SRC", "COPY_DST"];
                    let mut texture = |suffix| {
                        add(
                            &mut self.textures,
                            TextureDesc {
                                label: format!("{} {suffix}", r.key),
                                ..raw_texture(r, *size, format.name(), USAGE, None)
                            },
                        )
                    };
                    let a = texture("A");
                    let b = texture("B");
                    let zero = self.zero_texture(r, *size, format.name());
                    StepState::Texture { a, b, to, zero }
                }
                (ComputeShape::Buffer { .. }, Resource::Texture(_) | Resource::Sampler(_)) => {
                    return Err(Error::Internal(format!("compute \"{}\": expected buffer state", r.key)));
                }
                (ComputeShape::Texture { .. }, Resource::Buffer(_) | Resource::Sampler(_)) => {
                    return Err(Error::Internal(format!("compute \"{}\": expected texture state", r.key)));
                }
            });
        }
        let start_pipeline =
            self.compute_pipeline(format!("{} {}", c.label, steps.start), c.layout, c.module, &steps.start);
        let mut run = |pipe, side| {
            let (bindings, clear_buffers, clear_textures) = pairs(&state, side);
            RawDispatch {
                run: self.make_dispatch(p, c, pipe, &bindings),
                clear_buffers: Some(clear_buffers),
                clear_textures: Some(clear_textures),
            }
        };
        Ok(StepRuns {
            start: run(start_pipeline, Side::A),
            odd: run(c.pipeline, Side::B),
            even: run(c.pipeline, Side::A),
            count: steps.count,
            keep: steps.keep,
            history: state
                .into_iter()
                .map(|s| match s {
                    StepState::Buffer { a, b, to, bytes } => StepCopy::Buffer { a, b, to, bytes },
                    StepState::Texture { a, b, to, .. } => StepCopy::Texture { a, b, to },
                })
                .collect(),
        })
    }
}

#[derive(Clone, Copy)]
enum Side {
    A,
    B,
}

enum StepState {
    Buffer { a: Id, b: Id, to: Id, bytes: usize },
    Texture { a: Id, b: Id, to: Id, zero: Id },
}

fn pairs(state: &[StepState], side: Side) -> (Vec<Resource>, Vec<Id>, Vec<TextureCopy>) {
    let mut writes = Vec::new();
    let mut reads = Vec::new();
    let mut clear_buffers = Vec::new();
    let mut clear_textures = Vec::new();
    let order = |a, b| match side {
        Side::A => (a, b),
        Side::B => (b, a),
    };
    for s in state {
        match *s {
            StepState::Buffer { a, b, .. } => {
                let (to, from) = order(a, b);
                writes.push(Resource::Buffer(to));
                reads.push(Resource::Buffer(from));
                clear_buffers.push(to);
            }
            StepState::Texture { a, b, zero, .. } => {
                let (to, from) = order(a, b);
                writes.push(Resource::Texture(to));
                reads.push(Resource::Texture(from));
                clear_textures.push(TextureCopy { from: zero, to });
            }
        }
    }
    writes.extend(reads);
    (writes, clear_buffers, clear_textures)
}
