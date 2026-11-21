//! Frame entry points for hosts that also own presentation and timestamp readback.
use super::*;
pub use encode::{HostCallback, FrameOptions, FrameRecord, ModulePipelines, ModuleRecipe, PipelineCreates};

impl Runner {
    /// Prepare all cold pipeline misses together when the host already has its attachments.
    pub fn load_bundle_for_frame(&mut self, bundle: &Bundle, color: &wgpu::TextureView, depth: Option<&wgpu::TextureView>) -> Result<InstanceId> {
        self.load(bundle, Some(embed::Target { color, depth, caps: self.caps }))
    }

    pub fn prepare_frame(&mut self, id: InstanceId, view: &wgpu::TextureView) -> Result<()> {
        let (device, caps) = (self.device.clone(), self.caps);
        self.instance(id)?.prepare(&device, Some(&embed::Target { color: view, depth: None, caps }), 0)
    }

    pub fn load_stage_count(&mut self, id: InstanceId) -> Result<u32> {
        let instance = self.instance(id)?;
        Ok(instance.initializers.clocks.iter()
            .filter(|i| matches!(i.clock, mips::Clock::Load { done: false }))
            .map(|i| i.passes.len() as u32).sum())
    }

    pub fn encode_load(&mut self, id: InstanceId, enc: &mut wgpu::CommandEncoder, timer: Option<&wgpu::QuerySet>) -> Result<FrameRecord> {
        let device = self.device.clone();
        self.instance(id)?.record_initializers(&device, enc, timer, true)
    }

    pub fn image(&self, id: InstanceId, name: &str) -> Option<Arc<wgpu::Texture>> {
        let inst = self.instances.get(id.0 as usize)?;
        inst.tables.get(inst.initializers.images.get(name)?)?.texture()
    }

    pub fn pipeline_creates(&mut self, id: InstanceId) -> Result<&PipelineCreates> {
        Ok(&self.instance(id)?.compute.creates)
    }

    pub fn encode_frame(&mut self, id: InstanceId, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView,
        time: f64, options: FrameOptions<'_>) -> Result<FrameRecord> {
        self.record_frame(id, enc, view, None, time, loft_abi::AdvanceArgs::DT, options)
    }

    pub fn record_frame(&mut self, id: InstanceId, enc: &mut wgpu::CommandEncoder, color: &wgpu::TextureView,
        depth: Option<&wgpu::TextureView>, time: f64, dt: f32, options: FrameOptions<'_>) -> Result<FrameRecord> {
        let (device, caps) = (self.device.clone(), self.caps);
        let clock = encode::Clock { id, time, dt, to: (time * 120.).round() as u32, keep: 0, timer: None };
        self.instance(id)?.record_frame(&device, enc, clock, Some(&embed::Target { color, depth, caps }), options)
    }


    pub fn module_pipelines(&mut self, id: InstanceId, module: ModuleId) -> Result<(ModuleRecipe, ModulePipelines)> {
        self.instance(id)?.compute.recipe(module)
    }

    pub fn install_module_pipelines(&mut self, id: InstanceId, module: ModuleId, pipes: &ModulePipelines) -> Result<()> {
        self.instance(id)?.compute.install(module, pipes);
        Ok(())
    }
}
