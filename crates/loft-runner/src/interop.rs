use super::*;
impl Runner {
    pub fn bind_external_buffer(
        &mut self,
        _inst: InstanceId,
        _table: TableId,
        _buf: Arc<wgpu::Buffer>,
    ) -> Result<()> {
        Err(RunnerError::Unsupported(
            "external buffer binding (Phase 2)",
        ))
    }
    pub fn bind_external_texture(
        &mut self,
        _inst: InstanceId,
        _table: TableId,
        _tex: Arc<wgpu::Texture>,
    ) -> Result<()> {
        Err(RunnerError::Unsupported(
            "external texture binding (Phase 2)",
        ))
    }
    pub fn export_buffer(&self, inst: InstanceId, table: TableId) -> Option<Arc<wgpu::Buffer>> {
        self.instances
            .get(inst.0 as usize)?
            .tables
            .get(&table)?
            .buffer
            .clone()
    }
    pub fn export_texture(&self, _inst: InstanceId, _table: TableId) -> Option<Arc<wgpu::Texture>> {
        None
    }
}
