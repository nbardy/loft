use super::*;

pub trait ExecutionBackend: Sized {
    type Device;
    type Queue;
    type Encoder;
    type View;
    type Buffer;
    type Texture;
    fn attach(device: Self::Device, queue: Self::Queue, caps: HostCaps) -> Self;
    fn load_bundle(&mut self, bundle: &Bundle) -> Result<InstanceId>;
    fn upsert_rows(
        &mut self,
        inst: InstanceId,
        kind: KindId,
        start_row: u32,
        bytes: &[u8],
    ) -> Result<()>;
    fn set_row_count(&mut self, inst: InstanceId, kind: KindId, active_rows: u32) -> Result<()>;
    fn resize_table(&mut self, inst: InstanceId, table: TableId, new_capacity: u32) -> Result<()>;
    fn hot_swap_module(&mut self, inst: InstanceId, module: ModuleId, wgsl: &str) -> Result<()>;
    fn hot_swap_bundle(&mut self, inst: InstanceId, bundle: &Bundle) -> Result<()>;
    fn encode_into(
        &mut self,
        inst: InstanceId,
        encoder: &mut Self::Encoder,
        target: &Self::View,
        depth: Option<&Self::View>,
        time_s: f64,
        dt_s: f32,
    ) -> Result<FrameIdentity>;
    fn step_pass_cpu(
        &mut self,
        inst: InstanceId,
        pass: PassId,
        rows: std::ops::Range<u32>,
        ticks: u32,
    ) -> Result<&[u8]>;
    fn bind_external_buffer(
        &mut self,
        inst: InstanceId,
        table: TableId,
        buf: Self::Buffer,
    ) -> Result<()>;
    fn bind_external_texture(
        &mut self,
        inst: InstanceId,
        table: TableId,
        tex: Self::Texture,
    ) -> Result<()>;
    fn export_buffer(&self, inst: InstanceId, table: TableId) -> Option<Self::Buffer>;
    fn export_texture(&self, inst: InstanceId, table: TableId) -> Option<Self::Texture>;
}
pub struct WgpuBackend {
    pub(super) device: Arc<wgpu::Device>,
    pub(super) queue: Arc<wgpu::Queue>,
    pub caps: HostCaps,
    pub(super) instances: Vec<Instance>,
}
impl ExecutionBackend for WgpuBackend {
    type Device = Arc<wgpu::Device>;
    type Queue = Arc<wgpu::Queue>;
    type Encoder = wgpu::CommandEncoder;
    type View = wgpu::TextureView;
    type Buffer = Arc<wgpu::Buffer>;
    type Texture = Arc<wgpu::Texture>;
    fn attach(device: Self::Device, queue: Self::Queue, caps: HostCaps) -> Self {
        WgpuBackend::attach(device, queue, caps)
    }
    fn load_bundle(&mut self, bundle: &Bundle) -> Result<InstanceId> {
        WgpuBackend::load_bundle(self, bundle)
    }
    fn upsert_rows(
        &mut self,
        inst: InstanceId,
        kind: KindId,
        start_row: u32,
        bytes: &[u8],
    ) -> Result<()> {
        WgpuBackend::upsert_rows(self, inst, kind, start_row, bytes)
    }
    fn set_row_count(&mut self, inst: InstanceId, kind: KindId, active_rows: u32) -> Result<()> {
        WgpuBackend::set_row_count(self, inst, kind, active_rows)
    }
    fn resize_table(&mut self, inst: InstanceId, table: TableId, new_capacity: u32) -> Result<()> {
        WgpuBackend::resize_table(self, inst, table, new_capacity)
    }
    fn hot_swap_module(&mut self, inst: InstanceId, module: ModuleId, wgsl: &str) -> Result<()> {
        WgpuBackend::hot_swap_module(self, inst, module, wgsl)
    }
    fn hot_swap_bundle(&mut self, inst: InstanceId, bundle: &Bundle) -> Result<()> {
        WgpuBackend::hot_swap_bundle(self, inst, bundle)
    }
    fn encode_into(
        &mut self,
        inst: InstanceId,
        encoder: &mut Self::Encoder,
        target: &Self::View,
        depth: Option<&Self::View>,
        time_s: f64,
        dt_s: f32,
    ) -> Result<FrameIdentity> {
        WgpuBackend::encode_into(self, inst, encoder, target, depth, time_s, dt_s)
    }
    fn step_pass_cpu(
        &mut self,
        inst: InstanceId,
        pass: PassId,
        rows: std::ops::Range<u32>,
        ticks: u32,
    ) -> Result<&[u8]> {
        WgpuBackend::step_pass_cpu(self, inst, pass, rows, ticks)
    }
    fn bind_external_buffer(
        &mut self,
        inst: InstanceId,
        table: TableId,
        buf: Self::Buffer,
    ) -> Result<()> {
        WgpuBackend::bind_external_buffer(self, inst, table, buf)
    }
    fn bind_external_texture(
        &mut self,
        inst: InstanceId,
        table: TableId,
        tex: Self::Texture,
    ) -> Result<()> {
        WgpuBackend::bind_external_texture(self, inst, table, tex)
    }
    fn export_buffer(&self, inst: InstanceId, table: TableId) -> Option<Self::Buffer> {
        WgpuBackend::export_buffer(self, inst, table)
    }
    fn export_texture(&self, inst: InstanceId, table: TableId) -> Option<Self::Texture> {
        WgpuBackend::export_texture(self, inst, table)
    }
}
