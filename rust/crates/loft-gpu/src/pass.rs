//! Execute the ordered list on the caller's encoder. A previous read is a different physical image.
use super::{Canvas, Layers, Owner, View, rows_of, texel};
use loft_ir::Error;
use loft_ir::bundle::{Blend, Bundle, Clear, Cull, Shape, TexFormat};
use loft_ir::error::LoadFault;
use loft_ir::pass::{Count, Destination, Read, Run, Schedule};
use std::collections::HashSet;
use wgpu::*;

fn invalid(reason: impl Into<Box<str>>) -> Error {
    Error::Load(LoadFault::Invalid(reason.into()))
}

fn rows_bytes(b: &Bundle, r: u32) -> Option<u64> {
    match &b.resources.get(r as usize)?.shape {
        Shape::Rows { rows, lanes } => Some(u64::from(*rows) * u64::from(*lanes) * 16),
        _ => None,
    }
}
pub(super) fn buffer_roles(s: &Schedule) -> std::collections::HashMap<u32, BufferUsages> {
    let mut roles = std::collections::HashMap::new();
    for pass in &s.passes {
        if let Run::Raster { draws } = &pass.run {
            for d in draws {
                if let Some(r) = d.indices {
                    *roles.entry(r).or_insert(BufferUsages::empty()) |= BufferUsages::INDEX;
                }
                if let Count::Indirect(r) = d.instances {
                    *roles.entry(r).or_insert(BufferUsages::empty()) |= BufferUsages::INDIRECT;
                }
            }
        }
    }
    roles
}
pub(super) fn validate(s: &Schedule, b: &Bundle, device: &Device) -> Result<(), Error> {
    let mut available = HashSet::new();
    if !b.passes.is_empty() || s.passes.is_empty() {
        return Err(invalid("supply one nonempty ordered pass list"));
    }
    for im in &s.images {
        if !matches!(im.samples, 1 | 4) {
            return Err(invalid("choose 1 or 4 attachment samples"));
        }
    }
    for p in &s.passes {
        let bad = |why: &str| invalid(format!("pass {}: {why}", p.name));
        if p.module as usize >= b.modules.len() || !(1..=65536).contains(&p.repeat) {
            return Err(bad("supply a declared module and 1..65536 repeats"));
        }
        let image = |id: u32| s.images.get(id as usize).ok_or_else(|| bad("use a declared image index"));
        for r in &p.reads {
            let id = match *r {
                Read::Now(id) | Read::Previous(id) => id,
            };
            image(id)?;
            if matches!(r, Read::Now(_)) && !available.contains(&id) {
                return Err(bad("write the image before its Now read"));
            }
        }
        let mut outputs = HashSet::new();
        let mut samples = None;
        let mut depths = 0;
        for w in &p.writes {
            let (key, n) = match w.target {
                Destination::Output => (None, 1),
                Destination::Image(id) => {
                    let im = image(id)?;
                    if p.reads.contains(&Read::Now(id)) {
                        return Err(bad("read Previous or write a different image"));
                    }
                    if im.format == TexFormat::Depth32Float {
                        depths += 1;
                    }
                    if im.format == TexFormat::Rgba32Float
                        && w.blend != Blend::Opaque
                        && !device.features().contains(Features::FLOAT32_BLENDABLE)
                    {
                        return Err(bad(
                            "request FLOAT32_BLENDABLE on the device or accumulate into a different format",
                        ));
                    }
                    available.insert(id);
                    (Some(id), im.samples)
                }
            };
            if !outputs.insert(key) || samples.replace(n).is_some_and(|a| a != n) {
                return Err(bad("use distinct attachments of equal sample count"));
            }
        }
        if depths > 1 {
            return Err(bad("use a single depth attachment"));
        }
        match &p.run {
            Run::Raster { draws } => {
                if p.writes.is_empty() || draws.is_empty() {
                    return Err(bad("supply a draw and a destination"));
                }
                for d in draws {
                    if let Some(r) = d.indices
                        && !rows_bytes(b, r).is_some_and(|bytes| bytes >= u64::from(d.vertices) * 4)
                    {
                        return Err(bad("supply a u32 index table large enough for the requested draw"));
                    }
                    match d.instances {
                        Count::Fixed(_) => {}
                        Count::Rows(id)
                            if b.resources.get(id as usize).is_some_and(|r| matches!(r.shape, Shape::Rows { .. })) => {}
                        Count::Rows(_) => return Err(bad("use a row table for the draw count")),
                        Count::Indirect(r) => {
                            let minimum = if d.indices.is_some() { 20 } else { 16 };
                            if !rows_bytes(b, r).is_some_and(|bytes| bytes >= minimum) {
                                return Err(bad(
                                    "put the complete WebGPU indirect command in the first words of a row table",
                                ));
                            }
                        }
                    }
                }
            }
            Run::Compute { extent, .. } => {
                if extent.iter().any(|&v| v == 0 || v > device.limits().max_compute_workgroups_per_dimension) {
                    return Err(bad("choose a dispatch extent within device limits"));
                }
                for w in &p.writes {
                    let Destination::Image(id) = w.target else {
                        return Err(bad("compute writes a storage image; present it with a raster pass"));
                    };
                    let im = image(id)?;
                    if im.samples != 1
                        || im.format == TexFormat::Depth32Float
                        || w.clear != Clear::Load
                        || w.blend != Blend::Opaque
                    {
                        return Err(bad("compute writes single-sampled storage images with Load and Opaque"));
                    }
                }
            }
        }
    }
    Ok(())
}

enum Pipeline {
    Raster(Vec<(RenderPipeline, Command)>),
    Compute(ComputePipeline, [u32; 3]),
}
enum Command {
    Direct(u32, u32),
    Indirect(Buffer, u64),
    Indexed(Buffer, u64, u64, u32, u32),
    IndexedIndirect(Buffer, u64, u64, Buffer, u64),
}
impl Command {
    fn encode(&self, p: &mut RenderPass<'_>) {
        match self {
            Self::Direct(v, n) => p.draw(0..*v, 0..*n),
            Self::Indirect(buf, at) => p.draw_indirect(buf, *at),
            Self::Indexed(index, start, end, v, n) => {
                p.set_index_buffer(index.slice(*start..*end), IndexFormat::Uint32);
                p.draw_indexed(0..*v, 0, 0..*n);
            }
            Self::IndexedIndirect(index, start, end, buf, at) => {
                p.set_index_buffer(index.slice(*start..*end), IndexFormat::Uint32);
                p.draw_indexed_indirect(buf, *at);
            }
        }
    }
}
fn command(d: &loft_ir::pass::Draw, b: &Bundle, packs: &[Buffer]) -> Command {
    let table = |r| {
        let (buffer, offset) = b
            .packs
            .iter()
            .enumerate()
            .find_map(|(i, p)| p.holds.iter().find(|(id, _)| *id == r).map(|(_, o)| (&packs[i], 16 * o)))
            .expect("validated row resource");
        (buffer.clone(), offset, offset + rows_bytes(b, r).expect("validated table"))
    };
    let direct = |n| match d.indices.map(table) {
        Some((i, a, z)) => Command::Indexed(i, a, z, d.vertices, n),
        None => Command::Direct(d.vertices, n),
    };
    match d.instances {
        Count::Fixed(n) => direct(n),
        Count::Rows(r) => direct(rows_of(b, r)),
        Count::Indirect(r) => {
            let (buf, at, _) = table(r);
            match d.indices.map(table) {
                Some((i, a, z)) => Command::IndexedIndirect(i, a, z, buf, at),
                None => Command::Indirect(buf, at),
            }
        }
    }
}

struct Entry {
    pipe: Pipeline,
    layout: BindGroupLayout,
}
pub(super) struct Ordered {
    by_canvas: Vec<(TextureFormat, Vec<Entry>)>,
    images: Vec<[TextureView; 2]>,
    paired: Vec<bool>,
    size: [u32; 2],
    pub frame: u32,
    pub previous: Option<[u8; 32]>,
    iteration: BindGroup,
    stride: u32,
}

fn blend(b: Blend) -> Option<BlendState> {
    match b {
        Blend::Opaque => None,
        Blend::Alpha => Some(BlendState::ALPHA_BLENDING),
        Blend::Add => {
            let c = BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            };
            Some(BlendState { color: c, alpha: c })
        }
    }
}

impl Ordered {
    pub fn new(
        device: &Device,
        queue: &Queue,
        s: &Schedule,
        b: &Bundle,
        modules: &[ShaderModule],
        packs: &[Buffer],
        groups: [&BindGroupLayout; 2],
        canvases: &[Canvas],
        cache: Option<&PipelineCache>,
    ) -> Self {
        let stride = device.limits().min_uniform_buffer_offset_alignment.max(16);
        let repeats = s.passes.iter().map(|p| p.repeat).max().unwrap_or(1);
        let bytes = u64::from(stride) * u64::from(repeats);
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("pass iteration"),
            size: bytes,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut data = vec![0; bytes as usize];
        for i in 0..repeats {
            let at = (i * stride) as usize;
            data[at..at + 4].copy_from_slice(&i.to_le_bytes());
        }
        queue.write_buffer(&buffer, 0, &data);
        let iter_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::all(),
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: BufferSize::new(16),
                },
                count: None,
            }],
        });
        let iteration = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &iter_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: BufferSize::new(16),
                }),
            }],
        });
        let paired: Vec<bool> =
            (0..s.images.len()).map(|i| s.passes.iter().any(|p| p.reads.contains(&Read::Previous(i as u32)))).collect();
        let mut by_canvas = vec![];
        for c in canvases {
            if by_canvas.iter().any(|(fmt, _)| *fmt == c.color) {
                continue;
            }
            let entries = s
                .passes
                .iter()
                .map(|p| {
                    let compute = matches!(p.run, Run::Compute { .. });
                    let visibility = if compute { ShaderStages::COMPUTE } else { ShaderStages::VERTEX_FRAGMENT };
                    let mut binds = vec![];
                    for r in &p.reads {
                        let id = match *r {
                            Read::Now(id) | Read::Previous(id) => id,
                        };
                        let im = &s.images[id as usize];
                        let sample_type = match im.format {
                            TexFormat::Rgba16Uint => TextureSampleType::Uint,
                            TexFormat::Depth32Float => TextureSampleType::Depth,
                            _ => TextureSampleType::Float { filterable: false },
                        };
                        binds.push(BindGroupLayoutEntry {
                            binding: binds.len() as u32,
                            visibility,
                            ty: BindingType::Texture {
                                sample_type,
                                view_dimension: TextureViewDimension::D2,
                                multisampled: im.samples > 1,
                            },
                            count: None,
                        });
                    }
                    let mut targets = vec![];
                    let mut depth_stencil = None;
                    let mut samples = 1;
                    for w in &p.writes {
                        let fmt = match w.target {
                            Destination::Output => c.color,
                            Destination::Image(id) => {
                                let im = &s.images[id as usize];
                                samples = im.samples;
                                texel(im.format).0
                            }
                        };
                        if compute {
                            binds.push(BindGroupLayoutEntry {
                                binding: binds.len() as u32,
                                visibility,
                                ty: BindingType::StorageTexture {
                                    access: StorageTextureAccess::WriteOnly,
                                    format: fmt,
                                    view_dimension: TextureViewDimension::D2,
                                },
                                count: None,
                            });
                        } else if fmt.is_depth_stencil_format() {
                            depth_stencil = Some(DepthStencilState {
                                format: fmt,
                                depth_write_enabled: Some(true),
                                depth_compare: Some(CompareFunction::Less),
                                stencil: Default::default(),
                                bias: Default::default(),
                            });
                        } else {
                            targets.push(Some(ColorTargetState {
                                format: fmt,
                                blend: blend(w.blend),
                                write_mask: ColorWrites::ALL,
                            }));
                        }
                    }
                    let g2 = device
                        .create_bind_group_layout(&BindGroupLayoutDescriptor { label: Some(&p.name), entries: &binds });
                    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                        label: Some(&p.name),
                        bind_group_layouts: &[Some(groups[0]), Some(groups[1]), Some(&g2), Some(&iter_layout)],
                        immediate_size: 0,
                    });
                    let module = &modules[p.module as usize];
                    let pipe = match &p.run {
                        Run::Compute { entry, extent } => Pipeline::Compute(
                            device.create_compute_pipeline(&ComputePipelineDescriptor {
                                label: Some(&p.name),
                                layout: Some(&layout),
                                module,
                                entry_point: Some(entry),
                                compilation_options: Default::default(),
                                cache,
                            }),
                            *extent,
                        ),
                        Run::Raster { draws } => Pipeline::Raster(
                            draws
                                .iter()
                                .map(|d| {
                                    let pipe = device.create_render_pipeline(&RenderPipelineDescriptor {
                                        label: Some(&p.name),
                                        layout: Some(&layout),
                                        vertex: VertexState {
                                            module,
                                            entry_point: Some(&d.vertex),
                                            compilation_options: Default::default(),
                                            buffers: &[],
                                        },
                                        fragment: Some(FragmentState {
                                            module,
                                            entry_point: Some(&d.fragment),
                                            compilation_options: Default::default(),
                                            targets: &targets,
                                        }),
                                        primitive: PrimitiveState {
                                            cull_mode: match d.cull {
                                                Cull::None => None,
                                                Cull::Back => Some(Face::Back),
                                                Cull::Front => Some(Face::Front),
                                            },
                                            ..Default::default()
                                        },
                                        depth_stencil: depth_stencil.clone(),
                                        multisample: MultisampleState { count: samples, ..Default::default() },
                                        multiview_mask: None,
                                        cache,
                                    });
                                    (pipe, command(d, b, packs))
                                })
                                .collect(),
                        ),
                    };
                    Entry { pipe, layout: g2 }
                })
                .collect();
            by_canvas.push((c.color, entries));
        }
        let previous = paired.iter().any(|p| *p).then_some([0; 32]);
        Self { by_canvas, images: vec![], paired, size: [0, 0], frame: 0, previous, iteration, stride }
    }

    pub fn reset_history(&mut self) {
        self.size = [0, 0];
        self.frame = 0;
        self.previous = self.previous.map(|_| [0; 32]);
    }

    pub fn extent(&self) -> [u32; 2] {
        self.size
    }

    pub fn counts(&self) -> [usize; 2] {
        let mut total = [0, 0];
        for (_, entries) in &self.by_canvas {
            for e in entries {
                match &e.pipe {
                    Pipeline::Raster(ds) => total[0] += ds.len(),
                    Pipeline::Compute(..) => total[1] += 1,
                }
            }
        }
        total
    }

    pub fn encode(
        &mut self,
        device: &Device,
        enc: &mut CommandEncoder,
        view: &View,
        s: &Schedule,
        groups: [&BindGroup; 2],
        clock: Option<&QuerySet>,
    ) -> Result<(), Error> {
        if view.layers == Layers::Subject {
            return Err(Error::Usage("this pass list presents the whole output; supply an all-layers view".into()));
        }
        let output = match (view.canvas.samples, view.resolve) {
            (1, _) => view.color,
            (_, Some(v)) => v,
            _ => return Err(Error::Usage("ordered output requires a single-sampled view or resolve target".into())),
        };
        if view.size != self.size {
            self.images = s
                .images
                .iter()
                .enumerate()
                .map(|(i, im)| {
                    let storage = s.passes.iter().any(|p| {
                        matches!(p.run, Run::Compute { .. })
                            && p.writes.iter().any(|w| w.target == Destination::Image(i as u32))
                    });
                    let usage = TextureUsages::TEXTURE_BINDING
                        | TextureUsages::RENDER_ATTACHMENT
                        | if storage { TextureUsages::STORAGE_BINDING } else { TextureUsages::empty() };
                    let make = || {
                        device
                            .create_texture(&TextureDescriptor {
                                label: None,
                                size: Extent3d { width: view.size[0], height: view.size[1], depth_or_array_layers: 1 },
                                mip_level_count: 1,
                                sample_count: im.samples,
                                dimension: TextureDimension::D2,
                                format: texel(im.format).0,
                                usage,
                                view_formats: &[],
                            })
                            .create_view(&Default::default())
                    };
                    let a = make();
                    let b = if self.paired[i] { make() } else { a.clone() };
                    [a, b]
                })
                .collect();
            self.size = view.size;
            self.frame = 0;
        }
        let phase = (self.frame & 1) as usize;
        let entries = &self.by_canvas.iter().find(|(fmt, _)| *fmt == view.canvas.color).expect("loaded canvas").1;
        for (i, (p, entry)) in s.passes.iter().zip(entries).enumerate() {
            let now = |id: u32| &self.images[id as usize][phase];
            let mut reads: Vec<_> = p
                .reads
                .iter()
                .map(|r| match *r {
                    Read::Now(id) => now(id),
                    Read::Previous(id) => &self.images[id as usize][1 - phase],
                })
                .collect();
            if matches!(entry.pipe, Pipeline::Compute(..)) {
                for w in &p.writes {
                    let Destination::Image(id) = w.target else { unreachable!("validated") };
                    reads.push(now(id));
                }
            }
            let binds: Vec<_> = reads
                .iter()
                .enumerate()
                .map(|(k, v)| BindGroupEntry { binding: k as u32, resource: BindingResource::TextureView(v) })
                .collect();
            let g2 = device.create_bind_group(&BindGroupDescriptor {
                label: Some(&p.name),
                layout: &entry.layout,
                entries: &binds,
            });
            let start = Some(2 + 2 * i as u32);
            let end = Some(3 + 2 * i as u32);
            match &entry.pipe {
                Pipeline::Compute(pipe, extent) => {
                    let mut pass = enc.begin_compute_pass(&ComputePassDescriptor {
                        label: Some(&p.name),
                        timestamp_writes: clock.map(|query_set| ComputePassTimestampWrites {
                            query_set,
                            beginning_of_pass_write_index: start,
                            end_of_pass_write_index: end,
                        }),
                    });
                    pass.set_pipeline(pipe);
                    pass.set_bind_group(0, groups[0], &[0]);
                    pass.set_bind_group(1, groups[1], &[]);
                    pass.set_bind_group(2, &g2, &[]);
                    for rep in 0..p.repeat {
                        pass.set_bind_group(3, &self.iteration, &[rep * self.stride]);
                        pass.dispatch_workgroups(extent[0], extent[1], extent[2]);
                    }
                }
                Pipeline::Raster(draws) => {
                    let mut colors = vec![];
                    let mut depth = None;
                    for w in &p.writes {
                        let (v, is_depth, load) = match w.target {
                            Destination::Output => (output, false, view.owner == Owner::Caller),
                            Destination::Image(id) => {
                                (now(id), s.images[id as usize].format == TexFormat::Depth32Float, false)
                            }
                        };
                        let op = match (&w.clear, load) {
                            (Clear::Color(c), false) => {
                                let [r, g, b, a] = c.map(|n| f32::from_bits(n.0) as f64);
                                LoadOp::Clear(Color { r, g, b, a })
                            }
                            _ => LoadOp::Load,
                        };
                        if is_depth {
                            depth = Some(RenderPassDepthStencilAttachment {
                                view: v,
                                depth_ops: Some(Operations {
                                    load: match op {
                                        LoadOp::Clear(c) => LoadOp::Clear(c.r as f32),
                                        LoadOp::Load => LoadOp::Load,
                                        LoadOp::DontCare(_) => unreachable!(),
                                    },
                                    store: StoreOp::Store,
                                }),
                                stencil_ops: None,
                            });
                        } else {
                            colors.push(Some(RenderPassColorAttachment {
                                view: v,
                                depth_slice: None,
                                resolve_target: None,
                                ops: Operations { load: op, store: StoreOp::Store },
                            }));
                        }
                    }
                    let mut pass = enc.begin_render_pass(&RenderPassDescriptor {
                        label: Some(&p.name),
                        color_attachments: &colors,
                        depth_stencil_attachment: depth,
                        timestamp_writes: clock.map(|query_set| RenderPassTimestampWrites {
                            query_set,
                            beginning_of_pass_write_index: start,
                            end_of_pass_write_index: end,
                        }),
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_bind_group(0, groups[0], &[0]);
                    pass.set_bind_group(1, groups[1], &[]);
                    pass.set_bind_group(2, &g2, &[]);
                    for rep in 0..p.repeat {
                        pass.set_bind_group(3, &self.iteration, &[rep * self.stride]);
                        for (pipe, command) in draws {
                            pass.set_pipeline(pipe);
                            command.encode(&mut pass);
                        }
                    }
                }
            }
        }
        if self.previous.is_some() {
            self.frame = self.frame.wrapping_add(1);
        }
        Ok(())
    }
}
