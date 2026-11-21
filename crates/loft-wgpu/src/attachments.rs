use crate::error::*;
use loft_ir::*;
use wgpu::{CompareFunction, LoadOp, Operations, RenderBundleDepthStencil, StoreOp, TextureFormat, TextureView};

pub struct Color {
    pub view: TextureView,
    pub ops: Operations<wgpu::Color>,
}

pub struct Depth {
    pub view: TextureView,
    pub ops: Option<Operations<f32>>,
}

pub struct Target<'a> {
    pub color: &'a TextureView,
    pub depth: Option<&'a TextureView>,
    pub caps: HostCaps,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    pub colors: Vec<TextureFormat>,
    depth: Option<TextureFormat>,
    pub samples: u32,
    comparison: CompareFunction,
    depth_write: bool,
}

impl Canvas {
    pub fn depth_state(&self) -> Option<wgpu::DepthStencilState> {
        self.depth.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(self.depth_write),
            depth_compare: Some(self.comparison),
            stencil: Default::default(),
            bias: Default::default(),
        })
    }

    pub fn depth_layout(&self) -> Option<RenderBundleDepthStencil> {
        self.depth.map(|format| RenderBundleDepthStencil {
            format,
            depth_read_only: !self.depth_write,
            stencil_read_only: true,
        })
    }

    #[cfg(test)]
    fn color_ops(&self) -> Option<Operations<wgpu::Color>> {
        (!self.colors.is_empty()).then(preserve)
    }
    #[cfg(test)]
    fn depth_ops(&self) -> Option<Operations<f32>> {
        self.depth_write.then(preserve)
    }
}

#[cfg(test)]
fn preserve<T>() -> Operations<T> {
    Operations {
        load: LoadOp::Load,
        store: StoreOp::Store,
    }
}

fn comparison(convention: DepthConvention) -> CompareFunction {
    match convention {
        DepthConvention::StandardZeroToOne => CompareFunction::Less,
        DepthConvention::ReverseOneToZero => CompareFunction::Greater,
    }
}

pub struct Attachments {
    pub colors: Box<[TargetDesc]>,
    pub depth: Option<DepthDesc>,
    pub samples: u8,
}

impl Attachments {
    pub fn canvas<'a>(
        &self,
        target: &Target<'_>,
        internal: impl Fn(TableId) -> Option<(&'a TextureView, TextureDecl)>,
    ) -> Result<Canvas> {
        let samples = u32::from(self.samples);
        let attachment = |view: &TextureView, depth: bool| {
            let texture = view.texture();
            let format = texture.format();
            if texture.sample_count() != samples
                || texture.dimension() != wgpu::TextureDimension::D2
                || !texture.usage().contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
                || if depth {
                    !format.has_depth_aspect()
                } else {
                    format.is_depth_stencil_format()
                }
            {
                return Err(RunnerError::AbiMismatch);
            }
            Ok(format)
        };
        let colors = self
            .colors
            .iter()
            .map(|t| {
                let view = internal(t.table).map_or(target.color, |(view, _)| view);
                attachment(view, false)
            })
            .collect::<Result<Vec<_>>>()?;
        let depth_view = self
            .depth
            .as_ref()
            .map(|t| {
                internal(t.table)
                    .map(|(view, _)| view)
                    .or(target.depth)
                    .ok_or(RunnerError::AbiMismatch)
            })
            .transpose()?;
        let compare = if self.depth.as_ref().is_some_and(|t| internal(t.table).is_some()) {
            CompareFunction::Less
        } else {
            comparison(target.caps.depth_convention)
        };
        if self.colors.iter().any(|t| internal(t.table).is_none()) && self.samples != target.caps.msaa_samples {
            return Err(RunnerError::AbiMismatch);
        }
        Ok(Canvas {
            colors,
            depth: depth_view.map(|view| attachment(view, true)).transpose()?,
            samples,
            comparison: compare,
            depth_write: self.depth.as_ref().is_some_and(|t| t.write),
        })
    }

    pub fn views<'a>(
        &self,
        target: &Target<'_>,
        internal: impl Fn(TableId) -> Option<(&'a TextureView, TextureDecl)>,
        written: &mut std::collections::BTreeSet<TableId>,
    ) -> (Vec<Color>, Option<Depth>) {
        // Internal targets are cleared on their first writer each frame. Caller targets always load.
        let colors: Vec<_> = self
            .colors
            .iter()
            .map(|t| {
                let (view, clear) =
                    internal(t.table).map_or((target.color, None), |(view, decl)| (view, decl.clear));
                let load = match clear.filter(|_| written.insert(t.table)) {
                    Some([r, g, b, a]) => LoadOp::Clear(wgpu::Color { r, g, b, a }),
                    None => LoadOp::Load,
                };
                Color {
                    view: view.clone(),
                    ops: Operations {
                        load,
                        store: StoreOp::Store,
                    },
                }
            })
            .collect();
        let depth = self.depth.as_ref().map(|t| {
            let (view, clear) = match internal(t.table) {
                Some((view, decl)) => (view, decl.clear),
                None => (target.depth.expect("validated host depth"), None),
            };
            let depth_ops = t.write.then(|| Operations {
                load: clear
                    .filter(|_| written.insert(t.table))
                    .map_or(LoadOp::Load, |c| LoadOp::Clear(c[0] as f32)),
                store: StoreOp::Store,
            });
            Depth {
                view: view.clone(),
                ops: depth_ops,
            }
        });
        (colors, depth)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_color_and_depth_are_preserved_and_occlude_in_both_directions() {
        // CPU twin of the attachment load and depth test on one covered pixel.
        fn load<T: Copy>(ops: Operations<T>, old: T) -> T {
            assert_eq!(ops.store, StoreOp::Store);
            match ops.load {
                LoadOp::Load => old,
                op => panic!("host attachment was not loaded: {:?}", std::mem::discriminant(&op)),
            }
        }

        let host_color = wgpu::Color::GREEN;
        for (convention, near, far) in [
            (DepthConvention::StandardZeroToOne, 0.25, 0.75),
            (DepthConvention::ReverseOneToZero, 0.75, 0.25),
        ] {
            for samples in [1, 4] {
                for depth_write in [false, true] {
                    let canvas = Canvas {
                        colors: vec![TextureFormat::Rgba8Unorm],
                        depth: Some(TextureFormat::Depth32Float),
                        samples,
                        comparison: comparison(convention),
                        depth_write,
                    };
                    let state = canvas.depth_state().unwrap();
                    let layout = canvas.depth_layout().unwrap();
                    assert_eq!(layout.depth_read_only, !depth_write);
                    assert!(layout.stencil_read_only);
                    assert_eq!(state.depth_write_enabled, Some(depth_write));
                    assert_eq!(canvas.depth_ops().is_some(), depth_write);
                    for (z, visible) in [(near, true), (far, false), (0.5, false)] {
                        let old_z = canvas.depth_ops().map_or(0.5, |ops| load(ops, 0.5));
                        let passes = match state.depth_compare.unwrap() {
                            CompareFunction::Less => z < old_z,
                            CompareFunction::Greater => z > old_z,
                            other => panic!("unexpected host depth test {other:?}"),
                        };
                        let old_color = load(canvas.color_ops().unwrap(), host_color);
                        let color = if passes { wgpu::Color::RED } else { old_color };
                        let depth = if passes && depth_write { z } else { old_z };
                        assert_eq!(color, if visible { wgpu::Color::RED } else { host_color });
                        assert_eq!(depth, if visible && depth_write { near } else { 0.5 });
                    }
                }
            }
        }
    }
}
