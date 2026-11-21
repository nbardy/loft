use crate::error::{Result, RunnerError};
#[cfg(not(target_arch = "wasm32"))]
pub fn read_buffer(device: &wgpu::Device, queue: &wgpu::Queue, buffer: &wgpu::Buffer) -> Result<Vec<u8>> {
    if !buffer.usage().contains(wgpu::BufferUsages::COPY_SRC) { return Err(RunnerError::AbiMismatch); }
    read_copy(device, queue, buffer.size(), |encoder, read| {
        encoder.copy_buffer_to_buffer(buffer, 0, read, 0, Some(read.size()));
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_texture(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, level: u32) -> Result<Vec<u8>> {
    if !texture.usage().contains(wgpu::TextureUsages::COPY_SRC) || texture.sample_count() != 1
        || level >= texture.mip_level_count() || texture.format().block_dimensions() != (1, 1) {
        return Err(RunnerError::AbiMismatch);
    }
    let aspect = if texture.format() == wgpu::TextureFormat::Depth32Float {
        wgpu::TextureAspect::DepthOnly
    } else {
        wgpu::TextureAspect::All
    };
    let pixel = texture.format().block_copy_size(Some(aspect)).ok_or(RunnerError::AbiMismatch)?;
    let extent = texture.size().mip_level_size(level, texture.dimension());
    let used = extent.width.checked_mul(pixel).ok_or(RunnerError::Capacity)?;
    let pitch = used.checked_next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT).ok_or(RunnerError::Capacity)?;
    let size = u64::from(pitch) * u64::from(extent.height) * u64::from(extent.depth_or_array_layers);
    let padded = read_copy(device, queue, size, |encoder, read| {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { mip_level: level, aspect, ..texture.as_image_copy() },
            wgpu::TexelCopyBufferInfo { buffer: read, layout: wgpu::TexelCopyBufferLayout {
                offset: 0, bytes_per_row: Some(pitch), rows_per_image: Some(extent.height),
            }},
            extent,
        );
    })?;
    Ok(padded.chunks_exact(pitch as usize).flat_map(|row| row[..used as usize].iter().copied()).collect())
}

#[cfg(not(target_arch = "wasm32"))]
fn read_copy(device: &wgpu::Device, queue: &wgpu::Queue, size: u64,
    copy: impl FnOnce(&mut wgpu::CommandEncoder, &wgpu::Buffer)) -> Result<Vec<u8>> {
    if size > device.limits().max_buffer_size { return Err(RunnerError::Capacity); }
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None, size, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    copy(&mut encoder, &read);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    encoder.map_buffer_on_submit(&read, wgpu::MapMode::Read, .., move |r| { let _ = tx.send(r); });
    queue.submit([encoder.finish()]);
    device.poll(wgpu::PollType::wait_indefinitely()).map_err(|_| RunnerError::Readback)?;
    rx.recv().map_err(|_| RunnerError::Readback)?.map_err(|_| RunnerError::Readback)?;
    let mapped = read.get_mapped_range(..).map_err(|_| RunnerError::Readback)?;
    Ok(mapped.to_vec())
}
