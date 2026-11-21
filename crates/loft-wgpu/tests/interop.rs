use loft_wgpu::{error::RunnerError, interop::{TextureShape, check_buffer, check_texture, stale}};
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
    let shape = TextureShape {
        size: wgpu::Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        dim: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        samples: 1,
        mips: 1,
    };
    let binding = BindingType::StorageTexture {
        access: wgpu::StorageTextureAccess::WriteOnly,
        format: shape.format,
        view_dimension: V::D2,
    };
    let entry = wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: binding,
        count: None,
    };
    let check = |shape, usage| check_texture([&entry].into_iter(), shape, usage, wgpu::Features::empty());
    assert!(check(shape, T::STORAGE_BINDING).is_ok());
    assert_eq!(check(shape, T::TEXTURE_BINDING), Err(RunnerError::AbiMismatch));
    assert_eq!(
        check(TextureShape { samples: 4, ..shape }, T::STORAGE_BINDING),
        Err(RunnerError::AbiMismatch)
    );
    assert_eq!(
        check(
            TextureShape {
                format: wgpu::TextureFormat::R8Unorm,
                ..shape
            },
            T::STORAGE_BINDING
        ),
        Err(RunnerError::AbiMismatch)
    );
}
