//! The two things a frame needs from its view before the passes can read them (plan §1.5, §1.10):
//!
//! - **The camera.** One builder, `camera.wgsl`, turns a lens (`loft_ir::frame::LENS`) into the frame header's camera
//!   rows on the GPU. A bundle camera's lens entry writes the lens every frame (so it may move with time, params and
//!   inputs); a host look camera writes it from the CPU. A host matrix camera (`Aim::Matrices`, Keel's own
//!   view-projection) skips the builder.
//! - **The scene resolve.** Media, Screen covers and the ink composer read the opaque scene through the prelude
//!   (`loft_scene_depth`, `loft_post_colour`). Once per pass, before the first item that writes no depth after one that
//!   did, the host ends the render pass, copies the depth attachment into the r32float scene depth (a full-screen draw:
//!   a depth texture cannot be copied to a colour one) and the colour attachment into the scene colour, and goes on
//!   with a pass that loads both. `loft_scene_clearance` stays 0: its 8 × 8 tile reduction is not ported yet.
use loft_ir::Error;
use loft_ir::frame::{self, Camera};
use wgpu::{BindGroup, BindGroupLayout, Buffer, ShaderStages, TextureFormat};

const CAMERA: &str = include_str!("camera.wgsl");
const RESOLVE: &str = "@group(0) @binding(0) var depth: texture_depth_2d;
@vertex fn vs(@builtin(vertex_index) k: u32) -> @builtin(position) vec4<f32> {
  return vec4<f32>(f32(k == 1u) * 4.0 - 1.0, f32(k == 2u) * 4.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  return vec4<f32>(textureLoad(depth, vec2<i32>(p.xy), 0), 0.0, 0.0, 0.0);
}";
/// A multisampled depth's resolve: the nearest of each pixel's samples (TS SCENE_DEPTH_WGSL, which R0 drew with).
const RESOLVE_MS: &str = "@group(0) @binding(0) var depth: texture_depth_multisampled_2d;
@vertex fn vs(@builtin(vertex_index) k: u32) -> @builtin(position) vec4<f32> {
  return vec4<f32>(f32(k == 1u) * 4.0 - 1.0, f32(k == 2u) * 4.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  var z = 1.0;
  for (var s = 0u; s < textureNumSamples(depth); s++) { z = min(z, textureLoad(depth, vec2<i32>(p.xy), i32(s))); }
  return vec4<f32>(z, 0.0, 0.0, 0.0);
}";

/// Where the frame's camera comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    /// The host's matrices, written with the frame (`Loft::set_camera`).
    Matrices(Camera),
    /// A host look camera, built on the GPU (`Loft::set_look`).
    Look([f32; 4 * frame::LENS]),
    /// Bundle camera k, its lens evaluated and built on the GPU each frame (`Loft::use_camera`).
    Named(usize),
}

/// The camera builder's device objects: the lens and header buffers, the builder and one lens pipeline per bundle camera.
pub struct Cameras {
    lens: Buffer,
    head: Buffer,
    build: wgpu::ComputePipeline,
    build_group: BindGroup,
    lenses: Vec<wgpu::ComputePipeline>,
    lens_group: BindGroup,
}

fn storage(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    let ty = wgpu::BufferBindingType::Storage { read_only };
    let ty = wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None };
    wgpu::BindGroupLayoutEntry { binding, visibility: ShaderStages::COMPUTE, ty, count: None }
}

impl Cameras {
    /// `lenses` are the bundle cameras' (module, entry); their pipelines bind group 1 as render pipelines do (`packs`).
    pub fn new(
        device: &wgpu::Device,
        bgl0: &BindGroupLayout,
        packs: &BindGroupLayout,
        lenses: &[(&wgpu::ShaderModule, &str)],
        cache: Option<&wgpu::PipelineCache>,
    ) -> Cameras {
        let buffer = |label, size: u64, usage| {
            device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
        };
        let lens =
            buffer("loft lens", 16 * frame::LENS as u64, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let head =
            buffer("loft camera", frame::VIEWPORT as u64, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
        let layout = |entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("loft camera"), entries })
        };
        let (build_bgl, lens_bgl) = (layout(&[storage(0, true), storage(1, false)]), layout(&[storage(0, false)]));
        let pipeline = |groups: &[Option<&BindGroupLayout>], module: &wgpu::ShaderModule, entry: &str| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: groups,
                immediate_size: 0,
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache,
            })
        };
        let builder = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("loft camera"),
            source: wgpu::ShaderSource::Wgsl(CAMERA.into()),
        });
        let build = pipeline(&[Some(bgl0), Some(&build_bgl)], &builder, "loft_camera");
        let lenses = lenses.iter().map(|(m, e)| pipeline(&[Some(bgl0), Some(packs), Some(&lens_bgl)], m, e)).collect();
        let group = |layout: &BindGroupLayout, bufs: &[&Buffer]| {
            let entries: Vec<_> = bufs
                .iter()
                .enumerate()
                .map(|(k, b)| wgpu::BindGroupEntry { binding: k as u32, resource: b.as_entire_binding() })
                .collect();
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("loft camera"),
                layout,
                entries: &entries,
            })
        };
        let build_group = group(&build_bgl, &[&lens, &head]);
        let lens_group = group(&lens_bgl, &[&lens]);
        Cameras { lens, head, build, build_group, lenses, lens_group }
    }

    /// Records the camera of `aim` into `frame`'s header (its first `VIEWPORT` bytes), after the frame was written. `g0`
    /// and `packs` are the frame's group 0 (offset 0) and the render pipelines' group 1.
    pub fn encode(
        &self,
        queue: &wgpu::Queue,
        enc: &mut wgpu::CommandEncoder,
        aim: &Aim,
        (g0, packs): (&BindGroup, &BindGroup),
        frames: &[&Buffer],
        timestamp_writes: Option<wgpu::ComputePassTimestampWrites<'_>>,
    ) {
        let lens = match aim {
            Aim::Matrices(_) => return,
            Aim::Look(lens) => {
                queue.write_buffer(&self.lens, 0, &lens.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>());
                None
            }
            Aim::Named(k) => Some(*k),
        };
        {
            let mut pass =
                enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("loft camera"), timestamp_writes });
            pass.set_bind_group(0, g0, &[0]);
            if let Some(k) = lens {
                pass.set_pipeline(&self.lenses[k]);
                pass.set_bind_group(1, packs, &[]);
                pass.set_bind_group(2, &self.lens_group, &[]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            pass.set_pipeline(&self.build);
            pass.set_bind_group(1, &self.build_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        for f in frames {
            enc.copy_buffer_to_buffer(&self.head, 0, f, 0, Some(frame::VIEWPORT as u64));
        }
    }
}

/// The prelude's scene textures: depth (r32float), clearance (r8unorm, 0) and colour (the view's format), sized to the
/// view once a pass resolves into them, 1 × 1 (far, clear) before.
pub struct Scene {
    pub depth: wgpu::Texture,
    pub colour: wgpu::Texture,
    pub views: [wgpu::TextureView; 3],
    /// Bilinear, clamped: `post_color(uv)` (TS POST_WGSL's sampler).
    pub sampler: wgpu::Sampler,
}

impl Scene {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, size: [u32; 2], colour: TextureFormat) -> Scene {
        let make = |format: TextureFormat, size: [u32; 2], usage: wgpu::TextureUsages| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("loft scene"),
                size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: usage | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let depth = make(TextureFormat::R32Float, size, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let clearance = make(TextureFormat::R8Unorm, [1, 1], wgpu::TextureUsages::empty());
        let colour = make(colour, size, wgpu::TextureUsages::empty());
        // Far and clear until a resolve writes them.
        let fill = |t: &wgpu::Texture, texel: &[u8]| {
            let e = t.size();
            let bytes: Vec<u8> =
                texel.iter().copied().cycle().take(texel.len() * (e.width * e.height) as usize).collect();
            let layout = wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(e.width * texel.len() as u32),
                rows_per_image: None,
            };
            queue.write_texture(t.as_image_copy(), &bytes, layout, e);
        };
        fill(&depth, &1f32.to_le_bytes());
        let texel = colour.format().block_copy_size(None).expect("an uncompressed colour format") as usize;
        fill(&colour, &vec![0; texel]);
        fill(&clearance, &[0]);
        let views = [&depth, &clearance, &colour].map(|t| t.create_view(&Default::default()));
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("loft scene"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Scene { depth, colour, views, sampler }
    }

    pub fn fits(&self, size: [u32; 2], colour: TextureFormat) -> bool {
        let e = self.depth.size();
        [e.width, e.height] == size && self.colour.format() == colour
    }
}

/// The depth resolve's pipelines (a full-screen draw into the r32float scene depth), single- and multisampled.
pub struct Resolve {
    one: (BindGroupLayout, wgpu::RenderPipeline),
    many: (BindGroupLayout, wgpu::RenderPipeline),
}

fn resolve_pipeline(device: &wgpu::Device, multisampled: bool) -> (BindGroupLayout, wgpu::RenderPipeline) {
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("loft resolve"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled,
            },
            count: None,
        }],
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("loft resolve"),
        source: wgpu::ShaderSource::Wgsl(if multisampled { RESOLVE_MS } else { RESOLVE }.into()),
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("loft resolve"),
        layout: Some(&pl),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            targets: &[Some(TextureFormat::R32Float.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    (layout, pipeline)
}

impl Resolve {
    pub fn new(device: &wgpu::Device) -> Resolve {
        Resolve { one: resolve_pipeline(device, false), many: resolve_pipeline(device, true) }
    }

    /// Copies `colour` (the pass's picture, single-sampled: a multisampled view's resolve) and `depth` (the nearest
    /// sample when multisampled) into `scene`. Refused, naming the fix, when
    /// the attachments cannot be read: a caller's view must allow it.
    pub fn encode(
        &self,
        device: &wgpu::Device,
        enc: &mut wgpu::CommandEncoder,
        scene: &Scene,
        colour: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) -> Result<(), Error> {
        let (c, d) = (colour.texture(), depth.texture());
        let need = |t: &wgpu::Texture, u: wgpu::TextureUsages, what: &str| {
            match t.usage().contains(u) && (t.sample_count() == 1 || what == "depth") {
            true => Ok(()),
            false => Err(Error::Usage(
                format!(
                    "this view's passes read the opaque scene (a medium, a Screen cover or ink), so its {what} must be readable ({u:?}; a colour single-sampled, or resolved). Fix: create it so, or draw a bundle whose passes do not read the scene"
                )
                .into(),
            )),
        }
        };
        need(c, wgpu::TextureUsages::COPY_SRC, "colour")?;
        need(d, wgpu::TextureUsages::TEXTURE_BINDING, "depth")?;
        enc.copy_texture_to_texture(c.as_image_copy(), scene.colour.as_image_copy(), c.size());
        let (layout, pipeline) = if d.sample_count() > 1 { &self.many } else { &self.one };
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("loft resolve"),
            layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(depth) }],
        });
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("loft resolve"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &scene.views[0],
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(pipeline);
        rp.set_bind_group(0, &group, &[]);
        rp.draw(0..3, 0..1);
        Ok(())
    }
}
