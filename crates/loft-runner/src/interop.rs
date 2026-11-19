use super::*;
pub(super) use loft_std::interop::*;
impl Runner {
    pub fn bind_external_buffer(&mut self, inst: InstanceId, table: TableId, buf: Arc<wgpu::Buffer>) -> Result<()> {
        let limits = self.device.limits();
        let i = self.instance(inst)?;
        let t = i.tables.get_mut(&table).ok_or(RunnerError::UnknownResource)?;
        if !matches!(t.resource, Resource::Buffer(_)) { return Err(RunnerError::AbiMismatch); }
        validate_buffer(&i.decl, &t.decl, &buf, &limits, i.compute.resource_bindings(table))?;
        let epoch = next_epoch(i.resource_epoch)?;
        t.resource = Resource::Buffer(Some(buf));
        if t.decl.lifetime == TableLifetime::External { t.active = t.decl.capacity; }
        t.epoch = epoch;
        i.resource_epoch = epoch;
        Ok(())
    }
    pub fn declare_external_texture(&mut self, inst: InstanceId, table: TableId, desc: &wgpu::TextureDescriptor<'_>) -> Result<()> {
        let features = self.device.features();
        let i = self.instance(inst)?;
        let t = i.tables.get_mut(&table).ok_or(RunnerError::UnknownResource)?;
        if t.decl.lifetime != TableLifetime::External { return Err(RunnerError::AbiMismatch); }
        let TextureContract(shape, usage) = desc.into();
        check_texture(i.compute.resource_bindings(table),shape,usage,features)?;
        t.resource.declare(desc)
    }
    pub fn allocate_texture(&mut self, inst: InstanceId, table: TableId, desc: &wgpu::TextureDescriptor<'_>) -> Result<()> {
        let d = self.device.clone();
        let i = self.instance(inst)?;
        let t = i.tables.get_mut(&table).ok_or(RunnerError::UnknownResource)?;
        if t.decl.lifetime == TableLifetime::External || !matches!(t.resource,Resource::Texture(TextureBinding::Vacant)) {
            return Err(RunnerError::AbiMismatch);
        }
        let contract = TextureContract::from(desc);
        check_texture(i.compute.resource_bindings(table),contract.0,contract.1,d.features())?;
        let epoch = next_epoch(i.resource_epoch)?;
        t.resource = Resource::Texture(TextureBinding::Bound(contract, Arc::new(d.create_texture(desc))));
        t.epoch = epoch;
        i.resource_epoch = epoch;
        Ok(())
    }
    pub fn bind_external_texture(&mut self, inst: InstanceId, table: TableId, tex: Arc<wgpu::Texture>) -> Result<()> {
        let i = self.instance(inst)?;
        let t = i.tables.get_mut(&table).ok_or(RunnerError::UnknownResource)?;
        let epoch = next_epoch(i.resource_epoch)?;
        t.resource.adopt(tex)?;
        t.epoch = epoch;
        i.resource_epoch = epoch;
        Ok(())
    }
    pub fn export_buffer(&self, inst: InstanceId, table: TableId) -> Option<Arc<wgpu::Buffer>> {
        self.instances.get(inst.0 as usize)?.tables.get(&table)?.buffer().cloned()
    }
    pub fn export_texture(&self, inst: InstanceId, table: TableId) -> Option<Arc<wgpu::Texture>> {
        self.instances.get(inst.0 as usize)?.tables.get(&table)?.texture()
    }
}
