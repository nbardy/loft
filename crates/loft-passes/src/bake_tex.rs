//! Texture declarations lower into an ordinary compute pass and an optional mip pass.
use loft_ir::{DispatchExtent, ModuleId, PassBinding, PassDecl, PassId, PassRun, QueueClass, TableId};

pub use loft_parse::ast::TexDim;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Clock {
    Load,
    Dirty(Vec<usize>),
}

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

impl Bake {
    pub fn size(&self) -> [u32; 3] {
        match self.dim {
            TexDim::D2(w, h) => [w, h, 1],
            TexDim::D3(w, h, d) => [w, h, d],
            TexDim::Cube(n) => [n, n, 6],
        }
    }

    pub fn workgroup(&self) -> [u32; 3] {
        match self.dim {
            TexDim::D2(..) => [8, 8, 1],
            TexDim::Cube(_) => [8, 8, 6],
            TexDim::D3(..) => [4, 4, 4],
        }
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
