//! Read sample zero through a compute load, including for multisampled depth.
use super::*;

use wgpu::util::DeviceExt;

pub use loft_ir::PickResult;

#[path = "../../shared/visibility_ids.rs"]
mod visibility;
use visibility::visibility_ids;

pub(super) struct Source {
    vis: wgpu::Texture,
    depth: wgpu::Texture,
    inv_vp: [[f32; 4]; 4],
    pipeline: wgpu::ComputePipeline,
}

fn shader(multisampled: bool) -> String {
    let (vis, depth) = if multisampled {
        (
            "texture_multisampled_2d<u32>",
            "texture_depth_multisampled_2d",
        )
    } else {
        ("texture_2d<u32>", "texture_depth_2d")
    };
    format!(
        r#"
@group(0) @binding(0) var ids: {vis};
@group(0) @binding(1) var depths: {depth};
@group(0) @binding(2) var<uniform> px: vec2<u32>;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;
@compute @workgroup_size(1) fn read_pixel() {{
    let p = vec2<i32>(px);
    let v = textureLoad(ids, p, 0);
    output[0] = v.x | (v.y << 16u);
    output[1] = v.z | (v.w << 16u);
    output[2] = bitcast<u32>(textureLoad(depths, p, 0));
}}
"#
    )
}

impl Runner {
    /// Publish the textures and column-major inverse view-projection from the frame to pick.
    pub fn set_pick_source(
        &mut self,
        id: InstanceId,
        visibility: &wgpu::Texture,
        depth: &wgpu::Texture,
        inv_vp: [[f32; 4]; 4],
    ) -> Result<()> {
        use wgpu::{TextureDimension::D2, TextureFormat as F, TextureUsages as U};
        if visibility.format() != F::Rgba16Uint
            || !depth.format().has_depth_aspect()
            || visibility.dimension() != D2
            || depth.dimension() != D2
            || visibility.size() != depth.size()
            || depth.depth_or_array_layers() != 1
            || visibility.sample_count() != depth.sample_count()
            || !visibility.usage().contains(U::TEXTURE_BINDING)
            || !depth.usage().contains(U::TEXTURE_BINDING)
            || inv_vp.iter().flatten().any(|v| !v.is_finite())
        {
            return Err(RunnerError::AbiMismatch);
        }
        let device = self.device.clone();
        let source = &mut self.instance(id)?.pick_source;
        let pipeline = source
            .as_ref()
            .filter(|s| s.depth.sample_count() == depth.sample_count())
            .map(|s| s.pipeline.clone())
            .unwrap_or_else(|| {
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("sample zero pick"),
                    source: wgpu::ShaderSource::Wgsl(shader(depth.sample_count() > 1).into()),
                });
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("sample zero pick"),
                    layout: None,
                    module: &module,
                    entry_point: Some("read_pixel"),
                    compilation_options: Default::default(),
                    cache: None,
                })
            });
        *source = Some(Source {
            vis: visibility.clone(),
            depth: depth.clone(),
            inv_vp,
            pipeline,
        });
        Ok(())
    }

    pub fn pick_async(
        &mut self,
        id: InstanceId,
        px: [u32; 2],
        cb: Box<dyn FnOnce(PickResult) + Send>,
    ) -> Result<()> {
        let (device, queue) = (self.device.clone(), self.queue.clone());
        let source = self
            .instance(id)?
            .pick_source
            .as_ref()
            .ok_or(RunnerError::UnknownResource)?;
        let size = [source.vis.width(), source.vis.height()];
        if px[0] >= size[0] || px[1] >= size[1] {
            return Err(RunnerError::Capacity);
        }
        use wgpu::BufferUsages as U;
        let coord = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: px.map(u32::to_le_bytes).as_flattened(),
            usage: U::UNIFORM,
        });
        let output = allocate(&device, 12);
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("single pixel staging"),
            size: u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
            usage: U::MAP_READ | U::COPY_DST,
            mapped_at_creation: false,
        });
        let vis = source.vis.create_view(&Default::default());
        let depth = source.depth.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &source.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&vis),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: coord.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(&source.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        enc.copy_buffer_to_buffer(&output, 0, &staging, 0, Some(12));
        let pixels = staging.clone();
        let inv_vp = source.inv_vp;
        enc.map_buffer_on_submit(&staging, wgpu::MapMode::Read, ..12, move |mapped| {
            let result = if mapped.is_ok() {
                let bytes = pixels.get_mapped_range(..12);
                let result = bytes.as_ref().ok().map(|b| {
                    let w = |i| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
                    decode([w(0), w(4), w(8)], px, size, inv_vp)
                });
                drop(bytes);
                pixels.unmap();
                result
            } else {
                None
            };
            cb(result.unwrap_or_else(|| miss(f32::NAN)));
        });
        queue.submit([enc.finish()]);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn pick_sync(&mut self, id: InstanceId, px: [u32; 2]) -> Result<PickResult> {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.pick_async(
            id,
            px,
            Box::new(move |result| {
                let _ = tx.send(result);
            }),
        )?;
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|_| RunnerError::Unsupported("pick polling failed"))?;
        let result = rx
            .recv()
            .map_err(|_| RunnerError::Unsupported("pick readback failed"))?;
        if result.depth_ndc.is_nan() {
            return Err(RunnerError::Unsupported("pick readback failed"));
        }
        Ok(result)
    }
}

fn miss(depth_ndc: f32) -> PickResult {
    PickResult {
        hit: false,
        kind_id: 0,
        row_id: 0,
        payload: 0,
        depth_ndc,
        world_pos: [0.; 3],
    }
}

fn decode(
    [id, payload, z]: [u32; 3],
    px: [u32; 2],
    size: [u32; 2],
    inv: [[f32; 4]; 4],
) -> PickResult {
    let z = f32::from_bits(z);
    let (kind_id, row_id) = visibility_ids(id, payload);
    if kind_id == 0 {
        return miss(z);
    }
    let p = [
        2. * (px[0] as f32 + 0.5) / size[0] as f32 - 1.,
        1. - 2. * (px[1] as f32 + 0.5) / size[1] as f32,
        z,
        1.,
    ];
    let world: [f32; 4] = std::array::from_fn(|row| (0..4).map(|col| inv[col][row] * p[col]).sum());
    if !world.iter().all(|x| x.is_finite()) || world[3] == 0. {
        return miss(z);
    }
    PickResult {
        hit: true,
        kind_id,
        row_id,
        payload,
        depth_ndc: z,
        world_pos: std::array::from_fn(|i| world[i] / world[3]),
    }
}

#[cfg(test)]
#[test]
fn picking_shader_and_unprojection() {
    for msaa in [false, true] {
        let ir = naga::front::wgsl::parse_str(&shader(msaa)).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&ir)
        .unwrap();
    }
    let inv = [
        [2., 0., 0., 0.],
        [0., 4., 0., 0.],
        [0., 0., 8., 0.],
        [1., 2., 3., 2.],
    ];
    let hit = decode([(7 << 23) | 91, 42, 0.5f32.to_bits()], [0, 0], [2, 2], inv);
    assert!(hit.hit);
    assert_eq!((hit.kind_id, hit.row_id, hit.payload), (7, 91, 42));
    assert_eq!(hit.world_pos, [0., 2., 3.5]);
    assert!(!decode([0, 0, 0], [0, 0], [2, 2], inv).hit);
}
