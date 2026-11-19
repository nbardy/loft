//! Still mode (src/host/still.ts, term for term): a picture that averages `samples` samples of one instant, for
//! `loft render`, `frames` and `clip` with `--samples N` (and `--shutter s`, `--aperture r`).
//!
//! Sample k of N is a frame at its own time, its own sub-pixel offset and its own point on the lens, all functions of
//! (t, k) alone: time t + s (k + 0.5) / N; the projection moved inside the pixel by the k-th point of the (2, 3) Halton
//! sequence (index k + 1); the eye moved to the k-th point of a disc of radius r (Vogel's spiral) with the frustum
//! sheared so the plane through the camera's target stays put (an off-axis thin lens). Each sample's scene passes are
//! resolved as a frame's are and added into an f32 accumulator; the average goes where the scene's picture goes and the
//! post passes run once over it. Samples are drawn N - 1 down to 0, so the post passes read sample 0's depth. Every
//! sample plans its level of detail from sample 0's bounds, read once, with no memory of earlier frames.

use crate::camera::{self, Matrices};
use crate::manifest::{BoundsRead, Camera, Target};
use crate::plan::{self, Memory};
use crate::{bytes, Error, FrameStats, Loft, OUTPUT_FORMAT};
use std::time::Duration;

/// A still's samples, shutter (seconds after the frame's time) and lens radius (world units).
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
pub struct Still {
    pub samples: u32,
    pub shutter: f64,
    pub aperture: f64,
}

/// The input a still sets per sample for lights compiled sampled (src/artefact.ts `LIGHT_SAMPLE_INPUT`).
const LIGHT_SAMPLE_INPUT: &str = "loft_xi";

fn radical_inverse(k: u32, b: u32) -> f64 {
    let (mut f, mut r, mut i) = (1.0, 0.0, k);
    while i > 0 {
        f /= b as f64;
        r += f * (i % b) as f64;
        i /= b;
    }
    r
}

/// Sample k of n: its time after t, its pixel offset (pixels, x right, y down) and its point on the unit lens disc.
fn sample_of(k: u32, n: u32, s: Still) -> (f64, [f64; 2], [f64; 2]) {
    let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
    let (r, a) = (((k as f64 + 0.5) / n as f64).sqrt(), k as f64 * golden);
    (
        s.shutter * (k as f64 + 0.5) / n as f64,
        [radical_inverse(k + 1, 2) - 0.5, radical_inverse(k + 1, 3) - 0.5],
        [r * a.cos(), r * a.sin()],
    )
}

/// The k-th point of the R2 sequence (Roberts 2018): the light sample of a still's sample k.
fn r2(k: u32) -> [f32; 2] {
    let g = 1.324_717_957_244_746;
    [((0.5 + k as f64 / g) % 1.0) as f32, ((0.5 + k as f64 / (g * g)) % 1.0) as f32]
}

fn hypot3(a: f64, b: f64, c: f64) -> f64 {
    (a * a + b * b + c * c).sqrt()
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let l = hypot3(v[0], v[1], v[2]);
    [v[0] / l, v[1] / l, v[2] / l]
}

/// The 48-float frame uniform of sample k: the camera's eye on the lens, the frustum sheared to keep the target's
/// plane, then offset inside the pixel (`camera::frame_uniform` is the unmoved case).
#[allow(clippy::too_many_arguments)]
fn sample_uniform(
    c: &Camera,
    base: &Matrices,
    w: u32,
    h: u32,
    time: f64,
    k: u32,
    s: Still,
) -> Result<[f32; 48], Error> {
    let (_, pixel, lens) = sample_of(k, s.samples, s);
    let (eye, target) = match *c {
        Camera::Perspective { eye, target, .. } | Camera::Orthographic { eye, target, .. } => (eye, target),
    };
    let f = base.forward;
    let reference = if f[1].abs() > 0.999999 { [0.0, 0.0, -f[1].signum()] } else { [0.0, 1.0, 0.0] };
    let right = unit([
        f[1] * reference[2] - f[2] * reference[1],
        f[2] * reference[0] - f[0] * reference[2],
        f[0] * reference[1] - f[1] * reference[0],
    ]);
    let up = [right[1] * f[2] - right[2] * f[1], right[2] * f[0] - right[0] * f[2], right[0] * f[1] - right[1] * f[0]];
    let shift: [f64; 3] = std::array::from_fn(|i| s.aperture * (lens[0] * right[i] + lens[1] * up[i]));
    let add = |a: [f64; 3]| [a[0] + shift[0], a[1] + shift[1], a[2] + shift[2]];
    let moved = match *c {
        Camera::Perspective { fov_y, .. } => Camera::Perspective { eye: add(eye), target: add(target), fov_y },
        Camera::Orthographic { height, .. } => Camera::Orthographic { eye: add(eye), target: add(target), height },
    };
    let (wf, hf) = (w as f64, h as f64);
    let m = camera::matrices(&moved, wf / hf)?;
    let mut vp = m.view_proj;
    // Shear: a point of the target's plane (clip w = d) moved by -shift in view space lands back where it was, and the
    // whole picture moves by the pixel offset (NDC y is up, pixels y down). Added as multiples of clip w's row. The
    // projection's x and y scales are the lengths of view_proj's rows 0 and 1.
    let d = hypot3(target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]);
    let b = &base.view_proj;
    let (sx, sy) = (hypot3(b[0], b[4], b[8]), hypot3(b[1], b[5], b[9]));
    let ax = (s.aperture * lens[0] * sx) / d + (2.0 * pixel[0]) / wf;
    let ay = (s.aperture * lens[1] * sy) / d - (2.0 * pixel[1]) / hf;
    for col in 0..4 {
        vp[col * 4] += ax * vp[col * 4 + 3];
        vp[col * 4 + 1] += ay * vp[col * 4 + 3];
    }
    let sheared = Matrices { inv_view_proj: camera::invert(&vp)?, view_proj: vp, ..m };
    Ok(camera::frame_uniform(&sheared, w, h, time))
}

const ACCUMULATE_WGSL: &str = "
@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> sum: array<vec4<f32>>;
@compute @workgroup_size(8, 8) fn add(@builtin(global_invocation_id) g: vec3<u32>) {
  let size = textureDimensions(picture);
  if (g.x >= size.x || g.y >= size.y) { return; }
  let i = g.y * size.x + g.x;
  sum[i] = sum[i] + textureLoad(picture, vec2<i32>(g.xy), 0);
}";
const AVERAGE_WGSL: &str = "
@group(0) @binding(0) var<storage, read> sum: array<vec4<f32>>;
@group(0) @binding(1) var<uniform> shape: vec4<f32>;  // width, 1 / samples
@vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  return sum[u32(p.y) * u32(shape.x) + u32(p.x)] * shape.y;
}";

impl Loft {
    /// Draws a still of the set view into `target` (`OUTPUT_FORMAT`, `width` x `height`) and submits it; the caller
    /// copies `target` out after. Returns sample 0's plan's statistics. The time, inputs and plan memory are as before.
    pub fn draw_still(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        s: Still,
    ) -> Result<FrameStats, Error> {
        if width == 0 || height == 0 {
            return Err(Error::Size(width, height));
        }
        if s.samples == 0 {
            return Err(Error::Invalid("still: 0 samples; give at least 1".into()));
        }
        let camera = self.camera.ok_or_else(|| Error::Camera("no camera set".into()))?;
        if s.aperture > 0.0 && !matches!(camera, Camera::Perspective { .. }) {
            return Err(Error::Camera("an aperture needs a perspective camera".into()));
        }
        let base = camera::matrices(&camera, width as f64 / height as f64)?;
        // The scene's last pass is the last one that resolves; every pass after it is a post pass. Its resolve target
        // is where the scene's picture goes.
        let passes = &self.m.frame.passes;
        let last = passes
            .iter()
            .rposition(|p| p.color.as_ref().is_some_and(|c| c.resolve_target.is_some()))
            .ok_or_else(|| Error::Invalid("still: the frame has no pass that resolves the scene's picture".into()))?;
        let into = passes[last].color.as_ref().and_then(|c| c.resolve_target).expect("found above");
        let n_passes = passes.len();
        self.sized_for(device, (width, height));

        let accumulate_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("loft still accumulate"),
            source: wgpu::ShaderSource::Wgsl(ACCUMULATE_WGSL.into()),
        });
        let accumulate = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("loft still accumulate"),
            layout: None,
            module: &accumulate_module,
            entry_point: Some("add"),
            compilation_options: Default::default(),
            cache: None,
        });
        let average_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("loft still average"),
            source: wgpu::ShaderSource::Wgsl(AVERAGE_WGSL.into()),
        });
        let average = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("loft still average"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &average_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &average_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(OUTPUT_FORMAT.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sum = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("loft still sum"),
            size: width as u64 * height as u64 * 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shape = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("loft still shape"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&shape, 0, &bytes(&[width as f32, 1.0 / s.samples as f32, 0.0, 0.0]));
        // Where a sample's picture lands: an intermediate the post passes read, or (with no post) a texture of our own.
        let own = match into {
            Target::Output => Some(device.create_texture(&wgpu::TextureDescriptor {
                label: Some("loft still sample"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: OUTPUT_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })),
            Target::Texture { .. } => None,
        };
        let own_view = own.as_ref().map(|t| t.create_view(&Default::default()));
        let picture_view = match (into, &own_view) {
            (Target::Output, Some(v)) => v.clone(),
            (Target::Texture { texture }, _) => self.sized.as_ref().expect("sized above").views[texture].clone(),
            (Target::Output, None) => unreachable!("an output resolve makes its own texture"),
        };
        let accumulate_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("loft still accumulate"),
            layout: &accumulate.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&picture_view) },
                wgpu::BindGroupEntry { binding: 1, resource: sum.as_entire_binding() },
            ],
        });
        let average_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("loft still average"),
            layout: &average.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: sum.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: shape.as_entire_binding() },
            ],
        });

        let (time, held_inputs) = (self.time, self.inputs.clone());
        let xi = self.m.inputs.iter().find(|i| i.name == LIGHT_SAMPLE_INPUT).map(|i| i.slot);
        // One bounds read, sample 0's, made with no read in flight.
        let mut waited = self.harvest(device, true)?;
        self.time = time + sample_of(0, s.samples, s).0;
        let u0 = sample_uniform(&camera, &base, width, height, self.time, 0, s)?;
        self.write_frame(queue, &u0);
        let read = matches!(self.m.frame.bounds, BoundsRead::Read { .. });
        self.submit_pose(device, queue, read);
        waited += self.harvest(device, true)?;
        let bounds = if read { self.last_bounds.clone() } else { None };

        let mut stats = None;
        for k in (0..s.samples).rev() {
            let (dt, _, _) = sample_of(k, s.samples, s);
            self.time = time + dt;
            if let Some(slot) = xi {
                self.inputs[slot..slot + 2].copy_from_slice(&r2(k));
            }
            let u = sample_uniform(&camera, &base, width, height, self.time, k, s)?;
            self.write_frame(queue, &u);
            self.submit_pose(device, queue, false);
            let read = match &bounds {
                None => plan::Bounds::None,
                Some(floats) => plan::Bounds::Read { floats, at: &self.bounds_at },
            };
            plan::plan_frame(
                &self.m.plan,
                &base,
                width,
                height,
                self.cell_pixels,
                self.construction,
                &read,
                &mut Memory::new(),
                &mut self.detail,
                &mut self.plan,
            );
            queue.write_buffer(&self.buffers[self.m.frame.detail.buffer], 0, &bytes(&self.detail));
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("loft still sample") });
            if k == s.samples - 1 {
                encoder.clear_buffer(&sum, 0, None);
            }
            self.encode_passes(&mut encoder, own_view.as_ref().unwrap_or(target), 0..last + 1);
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("loft still accumulate"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&accumulate);
                pass.set_bind_group(0, &accumulate_group, &[]);
                pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
            }
            queue.submit([encoder.finish()]);
            if k == 0 {
                stats = Some(self.plan_stats(Duration::ZERO));
            }
        }
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("loft still average") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("loft still average"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if own.is_some() { target } else { &picture_view },
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&average);
            pass.set_bind_group(0, &average_group, &[]);
            pass.draw(0..3, 0..1);
        }
        // The post passes, once, over the average (they read sample 0's depth: it was drawn last).
        self.encode_passes(&mut encoder, target, last + 1..n_passes);
        queue.submit([encoder.finish()]);
        self.time = time;
        self.inputs = held_inputs;
        let stats = stats.expect("at least one sample");
        Ok(FrameStats { gpu_wait: waited, ..stats })
    }
}
