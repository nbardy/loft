//! Texture declarations lower into an ordinary compute pass and an optional mip pass.
use loft_ir::{DispatchExtent, ModuleId, PassBinding, PassDecl, PassId, PassRun, QueueClass, TableId};

pub use loft_parse::ast::TexDim;
pub use loft_emit::bake_tex::{Clock, SAMPLE_SAMPLER, SAMPLE_TEXTURE_PREFIX};
use loft_parse::ast::BakeDecl;
use crate::Prog;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Bake {
    pub name: String,
    pub dim: TexDim,
    pub format: naga::StorageFormat,
    pub clock: Clock,
    pub mips: bool,
    pub module: usize,
    /// Rows produced by one shader invocation. Archived programs bake one.
    #[serde(default = "one_row", skip_serializing_if = "one_row_only")]
    pub rows_per_invocation: u32,
}

fn one_row() -> u32 { 1 }
fn one_row_only(rows: &u32) -> bool { *rows == 1 }


pub(crate) fn bake_module(p: &Prog, types: &[loft_parse::ast::Ty], decl: &BakeDecl, module: usize) -> loft_ir::Res<(Bake, String)> {
    use naga::StorageFormat as F;
    let format = match decl.format.name.as_str() {
        "r8unorm" => F::R8Unorm,
        "r16float" => F::R16Float,
        "r32float" => F::R32Float,
        "rgba8unorm" => F::Rgba8Unorm,
        "rgba16float" => F::Rgba16Float,
        _ => unreachable!("parser checks the bake format"),
    };
    let (clock, rows_per_invocation, wgsl) = loft_emit::emit::bake_module(p, types, decl)?;
    let bake = Bake { name: decl.texel.name.name.clone(), dim: decl.dim, format, clock, mips: decl.mips, module, rows_per_invocation };
    Ok((bake, wgsl))
}

impl Bake {
    pub fn size(&self) -> [u32; 3] {
        loft_emit::bake_tex::size(self.dim)
    }

    pub fn workgroup(&self) -> [u32; 3] {
        loft_emit::bake_tex::workgroup(self.dim)
    }

    pub fn mip_count(&self) -> u32 {
        let [w, h, d] = self.size();
        if !self.mips { return 1; }
        let side = match self.dim {
            TexDim::D2(..) | TexDim::Cube(_) => w.max(h),
            TexDim::D3(..) => w.max(h).max(d),
        };
        u32::BITS - side.leading_zeros()
    }

    pub(crate) fn storage(&self) -> Result<loft_ir::execution::ImageStorage, String> {
        use loft_ir::execution::ImageStorage as S;
        use naga::StorageFormat as F;
        Ok(match self.format {
            F::R8Unorm => S::R8Unorm, F::R16Float => S::R16Float, F::R32Float => S::R32Float,
            F::Rgba8Unorm => S::Rgba8Unorm, F::Rgba16Float => S::Rgba16Float,
            _ => return Err("unsupported bake image format".into()),
        })
    }

    pub fn passes(&self, table: TableId, first: PassId) -> Vec<PassDecl> {
        let size = self.size();
        let mut group = self.workgroup();
        group[1] *= self.rows_per_invocation;
        let mut passes = vec![PassDecl {
            id: first, wave: 0, queue: QueueClass::Graphics,
            bindings: Box::new([PassBinding { group: 3, slot: 0, resource: table, access: loft_ir::BindAccess::StorageTex }]),
            run: PassRun::Compute {
                module: ModuleId(self.module as u32), entry: "bake".into(),
                dispatch: DispatchExtent::Fixed(std::array::from_fn(|i| size[i].div_ceil(group[i]))),
            },
        }];
        if self.mip_count() > 1 {
            passes.push(PassDecl {
                id: PassId(first.0 + 1), wave: 1, queue: QueueClass::Graphics, bindings: Box::new([]),
                run: PassRun::GenerateMips { texture: table },
            });
        }
        passes
    }
}
