use loft_wgpu::{error::RunnerError, interop::{TextureContract, check_buffer, check_texture, stale}};
use wgpu::{BindingType, BufferUsages as B, TextureUsages as T, TextureViewDimension as V};

#[test]
fn l23a_interop_checks_and_affected_groups_only() {
    assert!(stale(None, [0]));
    assert!(stale(Some(3), [2, 4]));
    assert!(!stale(Some(3), [1, 3]));
    assert!(!stale(Some(3), []));
    assert!(check_buffer(64, B::STORAGE, 64, B::STORAGE).is_ok());
    assert_eq!(
        check_buffer(60, B::STORAGE, 64, B::STORAGE),
        Err(RunnerError::AbiMismatch)
    );
    assert_eq!(
        check_buffer(64, B::COPY_DST, 64, B::STORAGE),
        Err(RunnerError::AbiMismatch)
    );
    let desc = wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        sample_count: 1,
        mip_level_count: 1,
        usage: T::STORAGE_BINDING,
        view_formats: &[],
    };
    let binding = BindingType::StorageTexture {
        access: wgpu::StorageTextureAccess::WriteOnly,
        format: desc.format,
        view_dimension: V::D2,
    };
    let entry = wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: binding,
        count: None,
    };
    let check = |desc: &wgpu::TextureDescriptor<'_>| {
        let TextureContract(shape, usage) = TextureContract::from(desc);
        check_texture([&entry].into_iter(), shape, usage, wgpu::Features::empty())
    };
    assert!(check(&desc).is_ok());
    for invalid in [
        wgpu::TextureDescriptor { usage: T::TEXTURE_BINDING, ..desc.clone() },
        wgpu::TextureDescriptor { sample_count: 4, ..desc.clone() },
        wgpu::TextureDescriptor { format: wgpu::TextureFormat::R8Unorm, ..desc },
    ] {
        assert_eq!(check(&invalid), Err(RunnerError::AbiMismatch));
    }
}
