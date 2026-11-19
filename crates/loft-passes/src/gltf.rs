//! Stage-0 glTF handle (06 §18). Asset indices address the whole bundle.
use loft_abi::MeshAsset;
use loft_ir::{Code, Diag, Res, Span};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct GltfScene {
    pub meshes: BTreeMap<String, MeshAsset>,
}

impl GltfScene {
    pub fn mesh(&self, name: &str, site: Span) -> Res<MeshAsset> {
        self.meshes.get(name).copied().ok_or_else(|| {
            Diag::new(
                Code::E602,
                site,
                format!("no imported mesh named {name:?}"),
                "use a mesh name from this glTF scene",
            )
        })
    }
}
