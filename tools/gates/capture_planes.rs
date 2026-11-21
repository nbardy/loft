// Included inside the scratch native module so the render uses the original shaders.
impl Loaded {
    pub fn picture_planes(&self, gpu: &Gpu) -> Result<(Vec<u8>, Vec<u8>), String> {
        let vis = self.picture_plane(gpu, self.vis.texture(), wgpu::TextureAspect::All, 8)?;
        let id = vis
            .chunks_exact(8)
            .flat_map(|pixel| pixel[..4].iter().copied())
            .collect();
        let depth =
            self.picture_plane(gpu, self.depth.texture(), wgpu::TextureAspect::DepthOnly, 4)?;
        Ok((id, depth))
    }

    fn picture_plane(
        &self,
        gpu: &Gpu,
        texture: &wgpu::Texture,
        aspect: wgpu::TextureAspect,
        pixel_size: u32,
    ) -> Result<Vec<u8>, String> {
        let (width, height) = self.size;
        let pitch = (width * pixel_size).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let data = buffer(
            &gpu.device,
            "picture plane",
            u64::from(pitch) * u64::from(height),
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                aspect,
                ..texture.as_image_copy()
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &data,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pitch),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        data.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = send.send(result);
        });
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        receive
            .recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let mapped = data.get_mapped_range(..).map_err(|e| e.to_string())?;
        Ok(mapped
            .chunks_exact(pitch as usize)
            .flat_map(|row| row[..(width * pixel_size) as usize].iter().copied())
            .collect())
    }
}
