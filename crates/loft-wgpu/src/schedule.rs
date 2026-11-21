//! Resolve and validate bundle declarations before creating pipelines.
use crate::{
    attachments::Attachments,
    bindings::{self, merge_bindings, slot},
    error::*,
    stepping::Fused,
};
use loft_ir::*;
use naga::ShaderStage;
use std::collections::{BTreeMap, BTreeSet};
use wgpu::{BindGroupLayoutEntry as Entry, BufferBindingType::Storage, ShaderStages as S};

pub type Slots = Vec<Vec<(Entry, TableId)>>;
pub enum Work {
    Dispatch(DispatchExtent),
    Ticks(Fused),
}
pub struct Draw {
    pub decl: IndirectDraw,
    pub index: Option<(TableId, wgpu::IndexFormat)>,
}
pub struct Raster {
    pub draws: Vec<Draw>,
    pub targets: Attachments,
    pub cull_front: bool,
}
pub enum Pass {
    Kernel {
        module: ModuleId,
        entry: String,
        slots: Slots,
        work: Work,
    },
    Draw(Raster, Slots),
    Mips(TableId),
}

pub enum Order {
    Run(PassId),
    Repeat(u32, Vec<Order>),
}
impl Order {
    pub fn visit(&self, f: &mut impl FnMut(PassId)) {
        match self {
            Self::Run(id) => f(*id),
            Self::Repeat(count, children) => {
                for _ in 0..*count {
                    for child in children {
                        child.visit(f);
                    }
                }
            }
        }
    }
}

pub struct Schedule {
    pub passes: BTreeMap<PassId, Pass>,
    pub order: Vec<Order>,
    pub outbox: Option<TableId>,
}

struct Shader {
    ir: naga::Module,
    info: naga::valid::ModuleInfo,
}
struct Linker<'a> {
    tables: BTreeMap<TableId, &'a TableDecl>,
    shaders: BTreeMap<ModuleId, Shader>,
    culling: &'a [PassId],
    passes: BTreeMap<PassId, Pass>,
    outbox: Option<TableId>,
}
impl Schedule {
    pub fn new(b: &Bundle, features: wgpu::Features) -> Result<Self> {
        if !crate::bundle::distinct_passes(&b.passes) {
            return Err(RunnerError::AbiMismatch);
        }
        let mut shaders = BTreeMap::new();
        for m in &b.modules {
            if m.target != ShaderTarget::Wgsl {
                return Err(RunnerError::Unsupported("shader target"));
            }
            let src = std::str::from_utf8(&m.bytes).map_err(|_| RunnerError::AbiMismatch)?;
            let ir = naga::front::wgsl::parse_str(src).map_err(|e| RunnerError::InvalidShader(e.to_string()))?;
            let info = wgpu_naga_bridge::create_validator(
                features,
                wgpu::DownlevelFlags::all(),
                naga::valid::ValidationFlags::all(),
            )
            .validate(&ir)
            .map_err(|e| RunnerError::InvalidShader(e.to_string()))?;
            if shaders.insert(m.id, Shader { ir, info }).is_some() {
                return Err(RunnerError::AbiMismatch);
            }
        }
        let mut link = Linker {
            tables: b.tables.iter().map(|t| (t.id, t)).collect(),
            shaders,
            culling: &b.resources.cull_front,
            passes: BTreeMap::new(),
            outbox: None,
        };
        let order = b.passes.iter().map(|p| link.pass(p)).collect::<Result<_>>()?;
        Ok(Self {
            passes: link.passes,
            order,
            outbox: link.outbox,
        })
    }
}

impl Linker<'_> {
    fn pass(&mut self, p: &PassDecl) -> Result<Order> {
        let mut slots = BTreeSet::new();
        if p.bindings.iter().any(|x| {
            x.group != x.access.group()
                || !self.tables.contains_key(&x.resource)
                || !slots.insert((x.group, x.slot))
        }) {
            return Err(RunnerError::AbiMismatch);
        }
        let pass = match &p.run {
            PassRun::Compute {
                module,
                entry,
                dispatch,
            } => self.dispatch(p, *module, entry, dispatch)?,
            PassRun::ComputeFusedTicks { module, entry, .. }
            | PassRun::ComputeNonDetTicks { module, entry, .. } => self.ticks(p, *module, entry)?,
            PassRun::RenderBundle {
                draws,
                color_targets,
                depth,
                msaa,
            } => self.draw(p, draws, color_targets, depth, *msaa)?,
            PassRun::PassLoop {
                max_iters,
                cond_buf,
                body,
            } => {
                if cond_buf.is_some() {
                    return Err(RunnerError::Unsupported("conditional loop requires shader lowering"));
                }
                return Ok(Order::Repeat(
                    *max_iters,
                    body.iter().map(|child| self.pass(child)).collect::<Result<_>>()?,
                ));
            }
            PassRun::GenerateMips { texture } => Pass::Mips(*texture),
        };
        self.passes.insert(p.id, pass);
        Ok(Order::Run(p.id))
    }
    fn dispatch(&mut self, p: &PassDecl, module: ModuleId, entry: &str, extent: &DispatchExtent) -> Result<Pass> {
        let valid = match extent {
            DispatchExtent::Fixed(_) => true,
            DispatchExtent::FromRowCount { table, workgroup_x } => {
                *workgroup_x > 0 && self.tables.contains_key(table)
            }
            DispatchExtent::Indirect { buf, offset_bytes } => {
                offset_bytes.is_multiple_of(4) && self.tables.contains_key(buf)
            }
        };
        if !valid {
            return Err(RunnerError::AbiMismatch);
        }
        self.kernel(p, module, entry, Work::Dispatch(extent.clone()))
    }
    fn ticks(&mut self, p: &PassDecl, module: ModuleId, entry: &str) -> Result<Pass> {
        let fused = Fused::reflect(&self.shader(module)?.ir, p)?;
        self.kernel(p, module, entry, Work::Ticks(fused))
    }
    fn shader(&self, id: ModuleId) -> Result<&Shader> {
        self.shaders.get(&id).ok_or(RunnerError::UnknownResource)
    }
    fn kernel(&mut self, p: &PassDecl, module: ModuleId, entry: &str, work: Work) -> Result<Pass> {
        let shader = self.shader(module)?;
        let used = shader.usage(ShaderStage::Compute, entry)?;
        let entries = bindings::reflect(&shader.ir, used, p)?;
        let outbox = shader
            .ir
            .global_variables
            .iter()
            .find(|(h, v)| !used[*h].is_empty() && v.name.as_deref() == Some("loft_outbox"));
        if let Some((_, var)) = outbox {
            let addr = var.binding.as_ref().ok_or(RunnerError::AbiMismatch)?;
            let slot = p
                .bindings
                .iter()
                .find(|b| {
                    u32::from(b.group) == addr.group
                        && b.slot == addr.binding
                        && b.access == BindAccess::ReadWriteStorage
                })
                .ok_or(RunnerError::AbiMismatch)?;
            if self
                .outbox
                .replace(slot.resource)
                .is_some_and(|old| old != slot.resource)
            {
                return Err(RunnerError::AbiMismatch);
            }
        }
        Ok(Pass::Kernel {
            module,
            entry: entry.into(),
            slots: groups(p, entries, S::COMPUTE)?,
            work,
        })
    }
    fn draw(
        &self,
        p: &PassDecl,
        draws: &[IndirectDraw],
        colors: &[TargetDesc],
        depth: &Option<DepthDesc>,
        samples: u8,
    ) -> Result<Pass> {
        if (colors.is_empty() && depth.is_none()) || !matches!(samples, 1 | 4) {
            return Err(RunnerError::AbiMismatch);
        }
        let mut reflected = BTreeMap::new();
        let draws = draws
            .iter()
            .map(|d| {
                if d.vertex.is_some() {
                    return Err(RunnerError::Unsupported(
                        "vertex attributes require a layout; use storage vertex pulling",
                    ));
                }
                if !self.tables.contains_key(&d.args) {
                    return Err(RunnerError::UnknownResource);
                }
                if !d.offset_bytes.is_multiple_of(4) {
                    return Err(RunnerError::AbiMismatch);
                }
                let index = d
                    .index
                    .map(|id| {
                        let t = self.tables.get(&id).ok_or(RunnerError::UnknownResource)?;
                        let format = match (t.format, t.stride_bytes) {
                            (ElemFormat::U16, 4) => wgpu::IndexFormat::Uint16,
                            (ElemFormat::U32, 4) => wgpu::IndexFormat::Uint32,
                            _ => return Err(RunnerError::AbiMismatch),
                        };
                        Ok::<_, RunnerError>((id, format))
                    })
                    .transpose()?;
                let shader = self.shader(d.module)?;
                let mut stage = |kind, name: &str, vis| {
                    merge_bindings(
                        &mut reflected,
                        bindings::reflect(&shader.ir, shader.usage(kind, name)?, p)?,
                        vis,
                    )
                };
                stage(ShaderStage::Vertex, &d.vertex_entry, S::VERTEX)?;
                if !colors.is_empty() || !d.fragment_entry.is_empty() {
                    stage(ShaderStage::Fragment, &d.fragment_entry, S::FRAGMENT)?;
                }
                Ok(Draw { decl: d.clone(), index })
            })
            .collect::<Result<_>>()?;
        let targets = Attachments {
            colors: colors.into(),
            depth: depth.clone(),
            samples,
        };
        Ok(Pass::Draw(
            Raster {
                draws,
                targets,
                cull_front: self.culling.contains(&p.id),
            },
            groups(p, reflected, S::FRAGMENT)?,
        ))
    }
}

impl Shader {
    fn usage(&self, stage: ShaderStage, name: &str) -> Result<&naga::valid::FunctionInfo> {
        let i = self
            .ir
            .entry_points
            .iter()
            .position(|e| e.name == name && e.stage == stage)
            .ok_or(RunnerError::AbiMismatch)?;
        Ok(self.info.get_entry_point(i))
    }
}
fn groups(p: &PassDecl, entries: BTreeMap<(u32, u32), Entry>, visibility: S) -> Result<Slots> {
    let mut groups = vec![vec![]; p.bindings.iter().map(|b| b.group).max().unwrap_or(0) as usize];
    for b in &p.bindings {
        let entry = match entries.get(&(u32::from(b.group), b.slot)) {
            Some(entry) => *entry,
            None if b.group <= 2 => {
                let mut e = slot(
                    b.slot,
                    Storage {
                        read_only: b.access == BindAccess::ReadStorage,
                    },
                );
                e.visibility = visibility;
                e
            }
            None => return Err(RunnerError::AbiMismatch),
        };
        groups[usize::from(b.group) - 1].push((entry, b.resource));
    }
    for group in &mut groups {
        group.sort_by_key(|(e, _)| e.binding);
    }
    Ok(groups)
}
