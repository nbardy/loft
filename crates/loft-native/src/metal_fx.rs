//! Temporal MetalFX on the same device and queue as the surrounding Loft passes.
#![allow(unsafe_code)]

use loft_runner::{Result, RunnerError};
use objc2::{
    msg_send,
    rc::{Retained, autoreleasepool},
    runtime::{AnyClass, AnyObject, Bool},
};
use wgpu::hal::api::Metal;

#[link(name = "MetalFX", kind = "framework")]
unsafe extern "C" {}

fn object<T>(value: &T) -> *const AnyObject {
    std::ptr::from_ref(value).cast()
}

pub fn supports(device: &wgpu::Device) -> bool {
    let Some(class) = AnyClass::get(c"MTLFXTemporalScalerDescriptor") else {
        return false;
    };
    // Only query a borrowed MTLDevice on the Metal backend.
    unsafe {
        let Some(hal) = device.as_hal::<Metal>() else {
            return false;
        };
        let supported: Bool = msg_send![class, supportsDevice: object(&**hal.raw_device())];
        supported.as_bool()
    }
}

pub struct TemporalScaler {
    device: wgpu::Device,
    effect: Retained<AnyObject>,
    _textures: [wgpu::Texture; 4],
}

impl TemporalScaler {
    pub fn new(
        device: &wgpu::Device,
        color: &wgpu::Texture,
        depth: &wgpu::Texture,
        motion: &wgpu::Texture,
        output: &wgpu::Texture,
    ) -> Result<Self> {
        if !supports(device) {
            return Err(RunnerError::Unsupported("MetalFX temporal scaler"));
        }
        let textures = [color, depth, motion, output];
        if textures.iter().any(|t| {
            t.dimension() != wgpu::TextureDimension::D2
                || t.depth_or_array_layers() != 1
                || t.sample_count() != 1
                || t.mip_level_count() != 1
        }) || color.size() != depth.size()
            || color.size() != motion.size()
            || textures[..3].contains(&output)
        {
            return Err(RunnerError::AbiMismatch);
        }
        let effect = autoreleasepool(|_| unsafe {
            let class =
                AnyClass::get(c"MTLFXTemporalScalerDescriptor").ok_or(RunnerError::AbiMismatch)?;
            let hal = device.as_hal::<Metal>().ok_or(RunnerError::AbiMismatch)?;
            let raw_device = object(&**hal.raw_device());
            let desc: Retained<AnyObject> = msg_send![class, new];
            let mut formats = [0usize; 4];
            let mut usages = [0usize; 4];
            let mut handles = Vec::with_capacity(4);
            for (i, tex) in textures.iter().enumerate() {
                let raw = tex.as_hal::<Metal>().ok_or(RunnerError::AbiMismatch)?;
                let raw = object(raw.raw_handle());
                let owner: *const AnyObject = msg_send![raw, device];
                let hazard: usize = msg_send![raw, hazardTrackingMode];
                let storage: usize = msg_send![raw, storageMode];
                // The separate Metal buffer uses the queue's normal tracked hazards.
                if owner != raw_device || hazard == 1 || (i == 3 && storage != 2) {
                    return Err(RunnerError::AbiMismatch);
                }
                formats[i] = msg_send![raw, pixelFormat];
                usages[i] = msg_send![raw, usage];
                // Retain each handle before dropping its guard. Re-entering wgpu while
                // holding texture guards can deadlock against the destruction lock.
                handles.push(Retained::retain(raw.cast_mut()).ok_or(RunnerError::AbiMismatch)?);
            }
            let _: () = msg_send![&desc, setColorTextureFormat: formats[0]];
            let _: () = msg_send![&desc, setDepthTextureFormat: formats[1]];
            let _: () = msg_send![&desc, setMotionTextureFormat: formats[2]];
            let _: () = msg_send![&desc, setOutputTextureFormat: formats[3]];
            let _: () = msg_send![&desc, setInputWidth: color.width() as usize];
            let _: () = msg_send![&desc, setInputHeight: color.height() as usize];
            let _: () = msg_send![&desc, setOutputWidth: output.width() as usize];
            let _: () = msg_send![&desc, setOutputHeight: output.height() as usize];
            let _: () = msg_send![&desc, setRequiresSynchronousInitialization: Bool::YES];
            let effect: Option<Retained<AnyObject>> =
                msg_send![&desc, newTemporalScalerWithDevice: raw_device];
            let effect =
                effect.ok_or(RunnerError::Unsupported("MetalFX texture formats or scale"))?;
            let required: [usize; 4] = [
                msg_send![&effect, colorTextureUsage],
                msg_send![&effect, depthTextureUsage],
                msg_send![&effect, motionTextureUsage],
                msg_send![&effect, outputTextureUsage],
            ];
            if usages
                .into_iter()
                .zip(required)
                .any(|(have, need)| have & need != need)
            {
                return Err(RunnerError::Unsupported("MetalFX texture usage"));
            }
            let _: () = msg_send![&effect, setColorTexture: &*handles[0]];
            let _: () = msg_send![&effect, setDepthTexture: &*handles[1]];
            let _: () = msg_send![&effect, setMotionTexture: &*handles[2]];
            let _: () = msg_send![&effect, setOutputTexture: &*handles[3]];
            let _: () = msg_send![&effect, setInputContentWidth: color.width() as usize];
            let _: () = msg_send![&effect, setInputContentHeight: color.height() as usize];
            let _: () = msg_send![&effect, setMotionVectorScaleX: 1.0f32];
            let _: () = msg_send![&effect, setMotionVectorScaleY: 1.0f32];
            let _: () = msg_send![&effect, setPreExposure: 1.0f32];
            Ok(effect)
        })?;
        Ok(Self {
            device: device.clone(),
            effect,
            _textures: textures.map(wgpu::Texture::clone),
        })
    }

    /// Record a temporal frame. Motion points to the previous sample in input-pixel units.
    ///
    /// # Safety
    /// Submit once in schedule order on this device's queue. Initialize these textures
    /// through wgpu before their first raw use and keep this scaler and the textures alive
    /// and undestroyed until GPU completion. Reset history on the first frame and after cuts.
    pub unsafe fn record(
        &mut self,
        jitter: [f32; 2],
        reverse_depth: bool,
        reset: bool,
    ) -> Result<wgpu::CommandBuffer> {
        unsafe {
            let effect = &self.effect;
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("MetalFX temporal"),
                });
            encoder.as_hal_mut::<Metal, _, _>(|hal| -> Result<()> {
                let raw = hal
                    .as_ref()
                    .and_then(|e| e.raw_command_buffer())
                    .ok_or(RunnerError::AbiMismatch)?;
                autoreleasepool(|_| {
                    let _: () = msg_send![effect, setDepthReversed: Bool::new(reverse_depth)];
                    let _: () = msg_send![effect, setReset: Bool::new(reset)];
                    let _: () = msg_send![effect, setJitterOffsetX: jitter[0]];
                    let _: () = msg_send![effect, setJitterOffsetY: jitter[1]];
                    let _: () = msg_send![effect, encodeToCommandBuffer: object(raw)];
                });
                Ok(())
            })?;
            Ok(encoder.finish())
        }
    }
}
