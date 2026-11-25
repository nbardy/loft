//! Convert the existing lowered schedule to the pass-derived bundle ABI.
use crate::{
    Compiled,
    program::{AuthoredRun, DispatchRows, Draw, Extent, FrameView, Pass, Program},
};
use loft_ir::{*, execution::{self, Execution, Initializer, Image}, precision::{Precision, Resolved, Role}};
use naga::{AddressSpace, ShaderStage, StorageAccess, TypeInner, valid};
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, String>;
type Resources = BTreeMap<Address, TableId>;
type Address = (u32, u32);
type Bindings = BTreeMap<(String, ShaderStage), Vec<(Address, PassBinding)>>;
/// Ids of the tick passes inside a lockstep loop: top-level pass ids are their bundle indices, so these sit far above.
const LOCKSTEP_PASSES: u32 = 1 << 30;

struct Builder {
    bundle: Bundle,
    bindings: Vec<Bindings>,
    baked_inputs: Resources,
    wave: u16,
}

/// `Role::Radiance` at `Resolved::F32` is `Rgba32Float`, and the shade modules declare their `hdr` store as
/// `rgba16float`. Retarget that one global to a new image type, not the shared one: baked textures authored as
/// `rgba16float` use the same naga type and keep their authored format.
fn retarget_hdr(ir: &mut naga::Module) {
    for (_, global) in ir.global_variables.iter_mut() {
        if global.name.as_deref() != Some("hdr") { continue; }
        let TypeInner::Image { dim, arrayed, class: naga::ImageClass::Storage { format: naga::StorageFormat::Rgba16Float, access } } =
            ir.types[global.ty].inner.clone() else { continue };
        global.ty = ir.types.insert(naga::Type { name: None, inner: TypeInner::Image { dim, arrayed,
            class: naga::ImageClass::Storage { format: naga::StorageFormat::Rgba32Float, access } } }, naga::Span::UNDEFINED);
    }
}

fn shader(mut ir: naga::Module, name: &str, nonce: Option<u32>, draw_words: u32, hdr: ImageFormat) -> Result<(String, Bindings)> {
    if matches!(hdr, ImageFormat::Rgba32Float) { retarget_hdr(&mut ir); }
    if let Some(nonce) = nonce
        && let Some((_, c)) = ir.constants.iter().find(|(_, c)| c.name.as_deref() == Some("LOFT_NONCE"))
    {
        ir.global_expressions[c.init] = naga::Expression::Literal(naga::Literal::U32(nonce));
    }
    if let Some((_, v)) = ir.overrides.iter().find(|(_, v)| v.name.as_deref() == Some("loft_draw_words")) {
        let init = v.init.ok_or("missing cull stride override default")?;
        ir.global_expressions[init] = naga::Expression::Literal(naga::Literal::U32(draw_words));
    }
    let mut slots = [0; 4];
    let mut resources = Vec::new();
    for (handle, global) in ir.global_variables.iter_mut() {
        let Some(binding) = &mut global.binding else {
            continue;
        };
        if binding.group == 0 {
            continue;
        }
        let access = match global.space {
            // Only the fixed group has a uniform slot in the bundle ABI.
            AddressSpace::Uniform => {
                global.space = AddressSpace::Storage {
                    access: StorageAccess::LOAD,
                };
                BindAccess::ReadStorage
            }
            AddressSpace::Storage { access } if access == StorageAccess::LOAD => {
                BindAccess::ReadStorage
            }
            AddressSpace::Storage { .. } => BindAccess::ReadWriteStorage,
            AddressSpace::Handle => match ir.types[global.ty].inner {
                TypeInner::Sampler { .. } => BindAccess::Sampler,
                TypeInner::Image {
                    class: naga::ImageClass::Storage { .. },
                    ..
                } => BindAccess::StorageTex,
                TypeInner::Image { .. } => BindAccess::SampledTex,
                TypeInner::BindingArray { .. } => BindAccess::BindlessTexArray,
                _ => return Err(format!("unsupported resource type in {}", name)),
            },
            _ => {
                return Err(format!(
                    "unsupported resource address space in {}",
                    name
                ));
            }
        };
        let original = (binding.group, binding.binding);
        let group = access.group();
        let slot = &mut slots[group as usize];
        binding.group = u32::from(group);
        binding.binding = *slot;
        resources.push((
            handle,
            original,
            PassBinding {
                group,
                slot: *slot,
                resource: TableId(0),
                access,
            },
        ));
        *slot += 1;
    }
    let info = valid::Validator::new(
        valid::ValidationFlags::all(),
        valid::Capabilities::default(),
    )
    .validate(&ir)
    .map_err(|e| format!("{}: {e:?}", name))?;
    let bindings = ir.entry_points
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let usage = info.get_entry_point(index);
                (
                    (entry.name.clone(), entry.stage),
                    resources
                        .iter()
                        .filter(|(handle, _, _)| !usage[*handle].is_empty())
                        .map(|(_, address, binding)| (*address, *binding))
                        .collect(),
                )
            })
            .collect();
    let code =
        naga::back::wgsl::write_string(&ir, &info, naga::back::wgsl::WriterFlags::empty())
            .map_err(|e| format!("{}: {e}", name))?;
    Ok((code, bindings))
}

/// An edited kind module is lowered like the original: its `hdr` store must match the bundle's radiance format.
pub fn replacement(source: &str, hdr: ImageFormat) -> Result<String> {
    let ir = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    Ok(shader(ir, "replacement", None, 4, hdr)?.0)
}

impl Builder {
    fn new(program: &Program, nonce: Option<u32>, draw_words: u32, hdr: ImageFormat) -> Result<Self> {
        let mut bundle = Bundle::default();
        let mut bindings = Vec::new();
        for (i, module) in program.modules.iter().enumerate() {
            let (code, reflected) = shader(module.naga.clone(), &module.name, nonce, draw_words, hdr)?;
            bindings.push(reflected);
            bundle.modules.push(ModuleBlob {
                id: ModuleId(i as u32),
                hash: *blake3::hash(code.as_bytes()).as_bytes(),
                target: ShaderTarget::Wgsl,
                bytes: code.into_bytes().into_boxed_slice(),
            });
        }
        Ok(Self { bundle, bindings, wave: 0, baked_inputs: Resources::new() })
    }

    fn native_world(&mut self, program: &Program) -> Result<()> {
        let kind_modules = program.kinds.iter().map(|kind| kind.module).filter(|&id| {
            program.modules[id].wgsl.contains("fn loft_tiled_node(")
                && program.modules[id].wgsl.contains("fn world_secondary(")
        }).collect::<Vec<_>>();
        if kind_modules.is_empty() || program.instances.is_empty() { return Ok(()); }
        let bounds = self.words(6, program.instances.iter()
            .flat_map(|instance| instance.lo.into_iter().chain(instance.hi).map(f32::to_bits)));
        self.baked_inputs.insert((3, u32::MAX), bounds);
        for index in kind_modules {
            let module = &mut self.bundle.modules[index];
            let code = std::str::from_utf8(&module.bytes).map_err(|e| e.to_string())?;
            let ir = naga::front::wgsl::parse_str(code).map_err(|e| e.emit_to_string(code))?;
            let slot = ir.global_variables.iter().filter_map(|(_, g)| g.binding)
                .filter(|b| b.group == 3).map(|b| b.binding + 1).max().unwrap_or(0);
            let start = code.find("fn world_secondary(").ok_or("missing World traversal entry")?;
            let end = start + code[start..].find("\n}").ok_or("missing World traversal body")? + 2;
            let mut query = include_str!("world_ray_query.wgsl").replace("RAY_QUERY_SLOT", &slot.to_string());
            let members = ir.types.iter().find_map(|(_, ty)| match &ty.inner {
                TypeInner::Struct { members, .. } if ty.name.as_deref() == Some("LoftRay") => Some(members),
                _ => None,
            }).filter(|members| members.len() == 4).ok_or("missing World ray type")?;
            // Naga may rename members such as t0 during portable WGSL emission.
            for (member, key) in members.iter().zip(["RAY_ORIGIN", "RAY_DIRECTION", "RAY_NEAR", "RAY_FAR"]) {
                query = query.replace(key, member.name.as_deref().ok_or("unnamed World ray field")?);
            }
            // Naga's WGSL writer cannot round-trip ray query instructions. The
            // portable module has already been canonicalized, including its slots.
            let code = format!("enable wgpu_ray_query;\n{}{}{}", &code[..start], query, &code[end..]);
            let ir = naga::front::wgsl::parse_str(&code).map_err(|e| e.emit_to_string(&code))?;
            valid::Validator::new(valid::ValidationFlags::all(), valid::Capabilities::default() | valid::Capabilities::RAY_QUERY)
                .validate(&ir).map_err(|e| e.emit_to_string(&code))?;
            module.hash = *blake3::hash(code.as_bytes()).as_bytes();
            module.bytes = code.into_bytes().into_boxed_slice();
            let bindings = self.bindings[index].get_mut(&("shade".into(), ShaderStage::Compute))
                .ok_or("missing tiled shade entry")?;
            bindings.push(((3, u32::MAX), PassBinding {
                group: 3, slot, resource: bounds, access: BindAccess::AccelerationStructure,
            }));
        }
        Ok(())
    }

    fn table(
        &mut self,
        stride_bytes: u32,
        capacity: u32,
        initial: Vec<u8>,
        lifetime: TableLifetime,
    ) -> TableId {
        let id = TableId(self.bundle.tables.len() as u32);
        self.bundle.tables.push(TableDecl {
            id,
            format: ElemFormat::U32,
            lifetime,
            stride_bytes,
            capacity,
            initial: initial.into_boxed_slice(),
        });
        id
    }

    fn words(&mut self, stride_words: u32, words: impl IntoIterator<Item = u32>) -> TableId {
        let bytes: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
        let stride = stride_words * 4;
        self.table(
            stride,
            bytes.len() as u32 / stride,
            bytes,
            TableLifetime::Static,
        )
    }

    fn transient(&mut self, stride: u32, capacity: u32) -> TableId {
        self.table(stride, capacity, vec![], TableLifetime::Transient)
    }

    fn external(&mut self) -> TableId {
        self.table(4, 0, vec![], TableLifetime::External)
    }

    fn texture(
        &mut self,
        format: ImageFormat,
        size: (u32, u32),
        clear: Option<[f64; 4]>,
    ) -> TableId {
        let table = self.external();
        self.bundle.resources.textures.push(TextureDecl {
            table,
            format,
            extent: [size.0, size.1, 1],
            samples: 1,
            clear,
        });
        table
    }

    fn pass(
        &mut self,
        run: PassRun,
        entries: &[(ModuleId, &str, ShaderStage)],
        resources: &Resources,
    ) -> Result<()> {
        let index = self.bundle.passes.len();
        let decl = self.decl(index.try_into().map_err(|_| "too many passes")?, run, entries, resources)?;
        self.bundle.passes.push(decl);
        Ok(())
    }

    /// A pass declaration with its entries' bindings resolved, not yet placed in the bundle.
    fn decl(
        &mut self,
        id: u32,
        run: PassRun,
        entries: &[(ModuleId, &str, ShaderStage)],
        resources: &Resources,
    ) -> Result<PassDecl> {
        let mut bindings = BTreeMap::new();
        for (module, name, stage) in entries {
            let decl = self.bindings[module.0 as usize]
                .get(&(name.to_string(), *stage))
                .ok_or_else(|| format!("module {} has no {stage:?} entry {name}", module.0))?;
            for (original, binding) in decl {
                let mut binding = *binding;
                binding.resource = resources.get(original).or_else(|| self.baked_inputs.get(original)).copied()
                    .ok_or_else(|| format!("entry {name} needs original binding {original:?}"))?;
                if bindings
                    .insert((binding.group, binding.slot), binding)
                    .is_some_and(|b| b != binding)
                {
                    return Err(format!("entry {name} has incompatible bindings"));
                }
            }
        }
        Ok(PassDecl {
            id: PassId(id),
            wave: self.wave,
            queue: QueueClass::Graphics,
            bindings: bindings.into_values().collect(),
            run,
        })
    }

    fn compute(
        &mut self,
        module: usize,
        name: &str,
        dispatch: DispatchExtent,
        resources: &Resources,
    ) -> Result<()> {
        let module = ModuleId(module as u32);
        self.pass(
            PassRun::Compute {
                module,
                entry: name.into(),
                dispatch,
            },
            &[(module, name, ShaderStage::Compute)],
            resources,
        )
    }

    fn draw(
        &mut self,
        module: usize,
        entries: (&str, &str),
        args: (TableId, u32, Option<TableId>),
        targets: (TableId, Option<TableId>),
        resources: &Resources,
    ) -> Result<()> {
        let module = ModuleId(module as u32);
        let (vertex, fragment) = entries;
        self.pass(
            PassRun::RenderBundle {
                draws: Box::new([IndirectDraw {
                    module,
                    vertex_entry: vertex.into(),
                    fragment_entry: fragment.into(),
                    args: args.0,
                    offset_bytes: args.1,
                    vertex: None,
                    index: args.2,
                }]),
                color_targets: Box::new([TargetDesc { table: targets.0 }]),
                depth: targets.1.map(|table| DepthDesc { table, write: true }),
                msaa: 1,
            },
            &[
                (module, vertex, ShaderStage::Vertex),
                (module, fragment, ShaderStage::Fragment),
            ],
            resources,
        )
    }
}

pub fn parameters(program: &Program, compiled: &Compiled) -> Result<Vec<ParamMeta>> {
    use crate::eval::{Eval, Val};
    let prog = compiled.prog();
    let names: Vec<_> = ["$sun", "$sky_zenith", "$sky_horizon"]
        .into_iter()
        .chain(prog.continuous_params().map(|(name, _)| name))
        .collect();
    if program.params.len() != 4 * names.len() {
        return Err("compiler parameter layout changed".into());
    }
    let mut eval = Eval::<f32>::new(&prog, &compiled.types, vec![], &[]);
    eval.luts = &compiled.luts;
    let mut number = |expr| {
        let value = match eval
            .expr(expr)
            .map_err(|_| "cannot evaluate parameter range")?
        {
            Val::S(x) => x,
            Val::U(x) => x as f32,
            Val::I(x) => x as f32,
            _ => return Err("expected a scalar parameter range"),
        };
        if !value.is_finite() {
            return Err("non-finite parameter range");
        }
        Ok(value)
    };
    names
        .into_iter()
        .zip(program.params.as_chunks::<4>().0.iter().copied())
        .enumerate()
        .map(|(i, (name, default))| {
            let range = prog
                .consts
                .get(name)
                .and_then(|c| c.range.as_ref())
                .map(|r| {
                    let lo = number(&r.lo)?;
                    let hi = if let Some(hi) = &r.hi {
                        number(hi)?
                    } else {
                        f32::MAX
                    };
                    let lo = if r.strict { lo.next_up() } else { lo };
                    Ok::<_, &str>(([lo, 0., 0., 0.], [hi, 0., 0., 0.]))
                })
                .transpose()?;
            Ok(ParamMeta {
                id: ParamId(i as u32),
                name: name.into(),
                ty: if i < 3 { IrTy::Vec4 } else { IrTy::F32 },
                offset_bytes: (i * 16) as u32,
                default,
                range,
            })
        })
        .collect()
}

/// Lower a checked program to the same bundle used by the standalone loader.
pub fn lower(
    program: &Program,
    compiled: &Compiled,
    size: (u32, u32),
    frame: impl Fn(bool, Option<([f32; 3], [f32; 3], u32)>) -> Vec<u8>,
) -> Result<Bundle> {
    lower_program(program, parameters(program, compiled)?, size, Options::default(), frame).map(|p| p.bundle)
}

#[derive(Default)]
pub struct Options {
    pub nonce: Option<u32>,
    pub overdraw: bool,
    /// Native motion sampling has its own absolute tick and readback API.
    pub picture_only: bool,
    pub hardware_ray_query: bool,
    /// The author's opt-out (`--precision f32`); the default decides each role's width.
    pub precision: Precision,
    /// The device's features, which decide whether a decided precision gets half arithmetic or half storage only.
    pub features: GpuFeatures,
}

pub struct Prepared {
    pub bundle: Bundle,
    pub instances: TableId,
    pub luts: TableId,
    pub visibility: TableId,
    pub depth: TableId,
    pub modules: Vec<ModuleId>,
    pub shadow: Option<([f32; 3], [f32; 3], u32)>,
    pub shadow_wave: Option<u16>,
    /// What the build ran with, including the recorded fallback when the adapter lacks `SHADER_F16`.
    pub precision: Resolved,
}

pub fn lower_program(
    program: &Program, params: Vec<ParamMeta>, size: (u32, u32), opts: Options,
    frame: impl Fn(bool, Option<([f32; 3], [f32; 3], u32)>) -> Vec<u8>,
) -> Result<Prepared> {
    let indexed = |kind: &crate::program::KindTable| {
        !opts.overdraw && program.modules[kind.module].naga.entry_points.iter().any(|e| e.name == "vs_grid")
    };
    let arg_words = if program.kinds.iter().any(indexed) { 5 } else { 4 };
    let precision = opts.precision.resolve(opts.features);
    // Overdraw counts layers into the HDR attachment, a debug picture, not radiance: it stays on the half format.
    let hdr_format = Role::Radiance.format(if opts.overdraw { Resolved::HalfStorage } else { precision });
    let mut b = Builder::new(program, opts.nonce, arg_words, hdr_format)?;
    b.bundle.params = params;
    if opts.hardware_ray_query && !opts.overdraw { b.native_world(program)?; }
    let mut execution = Execution::default();
    let mut kinds = Vec::new();
    let mut bases = Vec::new();
    let mut base = 0;
    for (i, kind) in program.kinds.iter().enumerate() {
        let rows = b.words(1, kind.rows.iter().map(|x| x.to_bits()));
        // Rows retain their stride for fused stepping.
        let table = &mut b.bundle.tables[rows.0 as usize];
        table.stride_bytes = kind.stride * 4;
        table.capacity = kind.n_rows;
        let params = b.words(4, [i as u32, base, kind.n_rows, 0]);
        bases.push(base);
        base += kind.n_rows;
        b.bundle.rows.insert(KindId(i as u32), rows);
        kinds.push((rows, params));
    }

    let luts = b.words(1, program.luts.iter().map(|x| x.to_bits()));
    // A table some read binds: the live rows of a drawn kind, or a static copy of a data kind's rows.
    let read_table = |b: &mut Builder, input: &DispatchRows| match input {
        DispatchRows::Drawable(i) => kinds[*i].0,
        DispatchRows::Table { n_rows, stride, words, .. } =>
            b.table(stride * 4, *n_rows, words.iter().flat_map(|f| f.to_le_bytes()).collect(), TableLifetime::Static),
    };
    for step in program.dispatches.iter().filter(|step| !opts.picture_only && !step.lockstep) {
        let mut motion = crate::fusion::motion_bundle(program, step, 0);
        let mut resources = Resources::new();
        let mut row_table = None;
        for table in motion.tables.drain(..) {
            let id = match table.id.0 {
                0 => {
                    let rows = match &step.input {
                        DispatchRows::Drawable(i) => kinds[*i].0,
                        DispatchRows::Table { .. } => b.table(
                            table.stride_bytes,
                            table.capacity,
                            table.initial.into_vec(),
                            table.lifetime,
                        ),
                    };
                    row_table = Some(rows);
                    rows
                }
                1 => b.words(6, [0; 6]),
                2 | 3 => b.transient(table.stride_bytes, table.capacity),
                4 => luts,
                // Reads of constant kinds; a row-local dispatch has no `previous`.
                n if (n as usize) < 5 + step.reads.len() => read_table(&mut b, &step.reads[n as usize - 5].input),
                _ => return Err("unknown fixed-step resource".into()),
            };
            let original = motion.passes[0].bindings.iter().find(|entry| entry.resource == table.id)
                .ok_or("fixed-step resource has no binding")?;
            resources.insert((u32::from(original.group), original.slot), id);
        }
        let module = ModuleId(step.module as u32);
        let rows = row_table.ok_or("fixed-step rows are missing")?;
        b.pass(
            crate::fusion::tick_run(step, module, rows),
            &[(module, &step.entry, ShaderStage::Compute)],
            &resources,
        )?;
        b.wave += 1;
    }
    // Kinds that read each other's rows: one loop of tick passes, advanced one tick at a time (fusion::motion_group).
    let group: Vec<&crate::program::Dispatch> = program.dispatches.iter().filter(|step| step.lockstep).collect();
    if !group.is_empty() && !opts.picture_only {
        let names: Vec<&str> = group.iter().map(|step| step.input.view(&program.kinds).expect("validated dispatch").0).collect();
        let rows: Vec<TableId> = group.iter().map(|step| read_table(&mut b, &step.input)).collect();
        let mut body = vec![];
        for (k, step) in group.iter().enumerate() {
            let (_, n, stride, _) = step.input.view(&program.kinds).expect("validated dispatch");
            let mut resources = Resources::new();
            for binding in &step.bindings {
                use crate::program::DispatchResource as R;
                let id = match binding.resource {
                    R::Rows => rows[k],
                    R::Arguments => b.words(6, [0; 6]),
                    R::Results => b.transient(step.checks.len() as u32 * 32, n),
                    R::Outbox => b.transient(4, 4096),
                    R::Luts => luts,
                    R::Read(j) => {
                        let read = &step.reads[j as usize];
                        match names.iter().position(|name| *name == read.kind) {
                            Some(i) => rows[i],
                            None => read_table(&mut b, &read.input),
                        }
                    }
                    R::Previous => b.transient(stride * 4, n),
                };
                resources.insert((binding.group, binding.slot), id);
            }
            let module = ModuleId(step.module as u32);
            if step.reads.iter().any(|read| read.previous) {
                let run = crate::fusion::ticks_run(step.tier, module, "snapshot", rows[k]);
                body.push(b.decl(LOCKSTEP_PASSES + body.len() as u32, run, &[(module, "snapshot", ShaderStage::Compute)], &resources)?);
            }
            let run = crate::fusion::tick_run(step, module, rows[k]);
            body.push(b.decl(LOCKSTEP_PASSES + body.len() as u32, run, &[(module, &step.entry, ShaderStage::Compute)], &resources)?);
        }
        let run = PassRun::PassLoop { max_iters: loft_abi::AdvanceArgs::MAX_TICKS, cond_buf: None, body: body.into() };
        let id = b.bundle.passes.len() as u32;
        b.bundle.passes.push(PassDecl { id: PassId(id), wave: b.wave, queue: QueueClass::Graphics, bindings: Box::new([]), run });
        b.wave += 1;
    }

    let count = program.kinds.len() as u32;
    let instances = b.words(
        8,
        program.instances.iter().flat_map(|i| {
            [
                i.lo[0].to_bits(),
                i.lo[1].to_bits(),
                i.lo[2].to_bits(),
                i.kind,
                i.hi[0].to_bits(),
                i.hi[1].to_bits(),
                i.hi[2].to_bits(),
                i.row,
            ]
        }).chain(program.instances.is_empty().then_some([0; 8]).into_iter().flatten()),
    );
    let bases = b.words(1, bases);
    let draw_args = b.words(
        arg_words,
        program.kinds.iter().flat_map(|k| [k.draw.vertices(), 0, 0, 0, 0].into_iter().take(arg_words as usize)),
    );
    let shadow_args = b.words(
        arg_words,
        program.kinds.iter().flat_map(|k| [k.draw.vertices(), k.n_rows, 0, 0, 0].into_iter().take(arg_words as usize)),
    );
    let shade_args = b.words(3, (0..count).flat_map(|_| [0, 1, 1]));
    let present_args = b.words(4, [3, 1, 0, 0]);
    let visible = b.transient(4, base);
    let all_rows = b.words(1, program.kinds.iter().flat_map(|k| 0..k.n_rows));
    let groups = [size.0.div_ceil(8), size.1.div_ceil(8), 1];
    let tiles = b.transient(4, count * groups[0] * groups[1]);
    let empty = Some([f64::from(u32::MAX), 0., 0., 0.]);
    let far = Some([1., 0., 0., 0.]);
    let vis = b.texture(Role::Integer.format(precision), size, empty);
    let depth = b.texture(Role::Depth.format(precision), size, far);
    let side = program.shadow_size.max(1);
    let shadow = b.texture(Role::Depth.format(precision), (side, side), far);
    let shadow_vis = b.texture(Role::Integer.format(precision), (side, side), empty);
    let sampler = b.external();
    b.bundle.resources.comparison_samplers.push(sampler);
    let hdr = b.texture(hdr_format, size, opts.overdraw.then_some([0.; 4]));
    let target = b.external();
    let shadow_bounds = program.passes.iter().find_map(|pass| match pass {
        Pass::Shadow(ids) => {
            let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
            for instance in program.instances.iter().filter(|i| ids.contains(&i.kind)) {
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(instance.lo[axis]);
                    hi[axis] = hi[axis].max(instance.hi[axis]);
                }
            }
            Some((lo, hi, side))
        }
        _ => None,
    });
    let mut frame_table = |sun| {
        let bytes = frame(sun, shadow_bounds);
        b.table(bytes.len() as u32, 1, bytes, TableLifetime::Static)
    };
    let main = frame_table(false);
    let alternate = shadow_bounds.map(|_| frame_table(true));
    let mut variants = Vec::new();
    let mut grid_data = BTreeMap::new();
    for (k, kind) in program.kinds.iter().enumerate().filter(|(_, kind)| indexed(kind)) {
        let Draw::Surface { nu, nv } = kind.draw else { return Err("indexed draw needs a parametric surface".into()); };
        let index = b.words(1, (0..nu * nv).flat_map(|cell| (0..6).map(move |corner| {
            let u = cell % nu + ((0x16_u32 >> corner) & 1);
            let v = cell / nu + ((0x34_u32 >> corner) & 1);
            v * (nu + 1) + u
        })));
        grid_data.insert(k, index);
    }

    let mut bakes = program.bakes.iter().collect::<Vec<_>>();
    bakes.sort_by_key(|bake| &bake.name);
    if !bakes.is_empty() {
        let sampler = b.external();
        b.baked_inputs.insert((3, 2), sampler);
    }
    for (slot, bake) in (3..).zip(bakes) {
        let texture = b.transient(4, 0);
        b.baked_inputs.insert((3, slot), texture);
        execution.initializers.push(append_bake(&mut b.bundle, bake, texture, &mut b.wave)?);
    }
    let mut present = Resources::from([((1, 0), hdr), ((1, 1), vis)]);
    if let Some(path) = &program.path {
        for (symbol, data) in path_words(program, path)? {
            present.insert(symbol, b.words(1, data));
        }
        if matches!(path.integrator, crate::emit::paths::Integrator::Spectral) {
            let bins = b.transient(4, 4 * 128 * 128);
            present.insert((2, 5), bins);
            b.bundle.resources.resets.push(bins);
            let module = program.passes.iter().find_map(|pass| if let Pass::Path(m) = pass { Some(*m) } else { None })
                .ok_or("a path-traced program has a path pass")?;
            b.compute(module, "photons", DispatchExtent::Fixed([1024, 1, 1]), &present)?;
            b.wave += 1;
        }
    }
    b.bundle.resources.resets.extend([draw_args, shade_args]);

    for pass in &program.passes {
        match pass {
            // The template's own (group, binding) addresses carry the frame views it bound by name.
            Pass::Authored(a) => {
                let resources = a.bindings.iter().map(|&(group, binding, view)| ((group, binding), match view {
                    FrameView::Instances => instances,
                    FrameView::KindBases => bases,
                    FrameView::DrawArgs => draw_args,
                    FrameView::Visible => visible,
                    FrameView::Visibility => vis,
                    FrameView::HdrStore | FrameView::HdrSample => hdr,
                    FrameView::ShadeArgs => shade_args,
                    FrameView::Tiles => tiles,
                })).collect::<Resources>();
                match &a.run {
                    AuthoredRun::Compute { entry, extent } => {
                        let groups = match *extent {
                            Extent::Groups(groups) => groups,
                            Extent::Instances { width } => [(program.instances.len() as u32).div_ceil(width), 1, 1],
                            Extent::Pixels { tile } => [size.0.div_ceil(tile[0]), size.1.div_ceil(tile[1]), 1],
                        };
                        b.compute(a.module, entry, DispatchExtent::Fixed(groups), &resources)?
                    }
                    AuthoredRun::FullScreen { vertex, fragment } => b.draw(
                        a.module, (vertex.as_str(), fragment.as_str()), (present_args, 0, None), (target, None), &resources)?,
                }
            }
            Pass::Vis(ids) | Pass::Shadow(ids) | Pass::Shade(ids) => {
                for &id in ids {
                    let k = id as usize;
                    let (rows, kp) = kinds[k];
                    let mut resources =
                        Resources::from([((1, 0), rows), ((1, 2), kp), ((1, 3), luts)]);
                    // Rows this kind's queries read (`K[i]`), in the slots its module declares.
                    for (j, &read) in program.kinds[k].reads.iter().enumerate() {
                        resources.insert((1, 4 + j as u32), kinds[read as usize].0);
                    }
                    if (matches!(pass, Pass::Vis(_) | Pass::Shadow(_)) || opts.overdraw)
                        && matches!(program.kinds[k].draw, Draw::Analytic | Draw::Traced)
                    {
                        b.bundle
                            .resources
                            .cull_front
                            .push(PassId(b.bundle.passes.len() as u32));
                    }
                    let index = grid_data.get(&k).copied();
                    let vs = if index.is_some() { "vs_grid" } else { "vs" };
                    let offset = id * arg_words * 4;
                    match pass {
                        Pass::Vis(_) => {
                            resources.insert((1, 1), visible);
                            b.draw(
                                program.kinds[k].module,
                                (vs, "fs"),
                                (draw_args, offset, index),
                                (vis, Some(depth)),
                                &resources,
                            )?;
                        }
                        Pass::Shadow(_) => {
                            variants.push((
                                PassId(b.bundle.passes.len() as u32),
                                alternate.ok_or("missing alternate frame")?,
                            ));
                            resources.insert((1, 1), all_rows);
                            b.draw(
                                program.kinds[k].module,
                                (vs, "fs"),
                                (shadow_args, offset, index),
                                (shadow_vis, Some(shadow)),
                                &resources,
                            )?;
                        }
                        Pass::Shade(_) if opts.overdraw => {
                            resources.insert((1, 1), visible);
                            execution.additive.push(PassId(b.bundle.passes.len() as u32));
                            b.draw(program.kinds[k].module, ("vs", "fs_overdraw"),
                                (draw_args, offset, None), (hdr, None), &resources)?;
                        }
                        Pass::Shade(_) => {
                            resources.extend([
                                ((1, 1), visible),
                                ((2, 0), vis),
                                ((2, 1), hdr),
                                ((2, 2), tiles),
                                ((2, 3), shadow),
                                ((2, 4), sampler),
                            ]);
                            b.compute(
                                program.kinds[k].module,
                                "shade",
                                DispatchExtent::Indirect {
                                    buf: shade_args,
                                    offset_bytes: id * 12,
                                },
                                &resources,
                            )?;
                        }
                        Pass::Authored(_) | Pass::Path(_) => unreachable!(),
                    }
                }
            }
            Pass::Path(module) => b.draw(
                *module,
                ("vs_present", "fs_present"),
                (present_args, 0, None),
                (target, None),
                &present,
            )?,
        }
        b.wave += 1;
    }
    let shadow_wave = variants.first().map(|(id, _)| b.bundle.passes[id.0 as usize].wave);
    b.bundle.resources.frame = Some(FrameData {
        main,
        variants,
        time_offset: 140,
    });
    execution.ranges = b.bundle.passes.iter().flat_map(|pass| {
        pass.bindings.iter().filter_map(move |binding| match binding.access {
            BindAccess::ReadStorage | BindAccess::ReadWriteStorage => Some((pass.id, binding)),
            BindAccess::SampledTex | BindAccess::StorageTex | BindAccess::BindlessTexArray | BindAccess::Sampler | BindAccess::AccelerationStructure => None,
        })
    }).map(|(pass, binding)| {
        let table = &b.bundle.tables[binding.resource.0 as usize];
        execution::BufferView { pass, group: binding.group, slot: binding.slot, offset: 0,
            size: (u64::from(table.capacity) * u64::from(table.stride_bytes)).max(4) }
    }).collect();
    b.bundle.tables.extend(execution.table());
    Ok(Prepared { bundle: b.bundle, instances, luts, visibility: vis, depth, shadow: shadow_bounds, shadow_wave,
        modules: program.kinds.iter().map(|k| ModuleId(k.module as u32)).collect(), precision })
}

fn append_bake(bundle: &mut Bundle, bake: &crate::bake_tex::Bake, texture: TableId, wave: &mut u16) -> Result<Initializer> {
    let first = PassId(bundle.passes.len() as u32);
    let mut passes = bake.passes(texture, first);
    let ids = passes.iter().map(|p| p.id).collect();
    for pass in &mut passes { pass.wave = *wave; *wave = wave.checked_add(1).ok_or("too many pass waves")?; }
    bundle.passes.extend(passes);
    use crate::bake_tex::{Clock, TexDim};
    let dimension = match bake.dim {
        TexDim::D2(..) => execution::Dimension::Plane,
        TexDim::D3(..) => execution::Dimension::Volume,
        TexDim::Cube(_) => execution::Dimension::Cube,
    };
    let clock = match &bake.clock {
        Clock::Load => execution::Clock::Load,
        Clock::Dirty(words) => execution::Clock::Dirty(words.clone()),
    };
    Ok(Initializer { name: bake.name.clone(), passes: ids, clock,
        image: Some(Image { table: texture, dimension, extent: bake.size(), levels: bake.mip_count(), format: bake.storage()? }) })
}

pub(super) fn compute_bakes(program: &Program, tables: &BTreeMap<String, TableId>, bundle: &mut Bundle) -> Result<()> {
    let mut execution = Execution::default();
    let mut wave = bundle.passes.last().map_or(Ok(0), |pass| pass.wave.checked_add(1).ok_or("too many pass waves"))?;
    for bake in &program.bakes {
        // A compute bundle already declares the sampled slot. Its initializer
        // makes that table runner-owned just as in a render bundle.
        let Some(&texture) = tables.get(&bake.name) else { continue };
        bundle.tables.iter_mut().find(|table| table.id == texture)
            .ok_or("unknown compute bake image")?.lifetime = TableLifetime::Transient;
        let original = &program.modules[bake.module];
        let (code, _) = shader(original.naga.clone(), &original.name, None, 4, ImageFormat::Rgba16Float)?;
        let mut bake = bake.clone();
        bake.module = bundle.modules.len();
        bundle.modules.push(ModuleBlob {
            id: ModuleId(bake.module as u32), target: ShaderTarget::Wgsl,
            hash: *blake3::hash(code.as_bytes()).as_bytes(), bytes: code.into_bytes().into_boxed_slice(),
        });
        execution.initializers.push(append_bake(bundle, &bake, texture, &mut wave)?);
    }
    bundle.tables.extend(execution.table());
    Ok(())
}

fn append_bvh(nodes: &mut Vec<u32>, leaves: &mut Vec<u32>, tree: &crate::bvh::Bvh2) -> Result<u32> {
    let node_base = u32::try_from(nodes.len() / 8).map_err(|_| "World BVH node address overflow")?;
    let leaf_base = u32::try_from(leaves.len() / 2).map_err(|_| "World BVH leaf address overflow")?;
    let root = if tree.nodes().is_empty() { 0 } else { node_base.checked_add(1).ok_or("World root overflow")? };
    for n in tree.nodes() {
        nodes.extend(n.lo.map(f32::to_bits));
        let base = if n.count == 0 { node_base } else { leaf_base };
        nodes.push(n.left_or_first.checked_add(base).ok_or("World BVH offset overflow")?);
        nodes.extend(n.hi.map(f32::to_bits));
        nodes.push(n.count);
    }
    leaves.extend(tree.leaves().iter().flat_map(|r| [r.kind_id, r.row_id]));
    Ok(root)
}

fn path_words(program: &Program, path: &crate::emit::paths::PathTrace) -> Result<[(Address, Vec<u32>); 5]> {
    let p = program;
    let mut words = vec![0u32; 1 + 2 * p.kinds.len()];
    let mut nodes = Vec::new();
    let mut leaves = Vec::new();
    let word = |n: usize| u32::try_from(n).map_err(|_| "World arena exceeds its word address range");
    let media_count = word(path.media.len())?;
    let mut world_data = vec![word(path.worlds.len())?, media_count];
    for world in &path.worlds {
        let instances = p.instances.iter().filter(|r| world.kinds.contains(&r.kind)).copied().collect::<Vec<_>>();
        let tree = crate::bvh::Bvh2::build(&instances).map_err(|e| format!("invalid world BVH: {e:?}"))?;
        let root = append_bvh(&mut nodes, &mut leaves, &tree)?;
        world_data.extend([root, if world.media { media_count } else { 0 }]);
    }
    world_data.extend(path.media.as_flattened().as_flattened().iter().map(|v| v.to_bits()));
    for (i, kind) in p.kinds.iter().enumerate() {
        words[1 + i * 2] = word(words.len())?;
        words[2 + i * 2] = kind.stride;
        words.extend(kind.rows.iter().map(|v| v.to_bits()));
    }
    words[0] = word(words.len())?;
    words.extend(world_data);
    if nodes.is_empty() { nodes.resize(8, 0); }
    if leaves.is_empty() { leaves.resize(2, 0); }
    Ok([
        ((2, 0), words), ((2, 1), nodes), ((2, 4), leaves),
        ((2, 2), p.luts.iter().map(|v| v.to_bits()).collect()),
        ((2, 3), path.lamps.as_flattened().as_flattened().iter().map(|v| v.to_bits()).collect()),
    ])
}

/// Declare the compiler's GPU twin with the same fixed frame ABI and pass-derived groups.
pub fn lower_twin(ir: naga::Module, lut: &[f32], kind: [u32; 4], rows_bytes: u64,
    params: Vec<ParamMeta>, frame: Vec<u8>, samples: &[f32]) -> Result<(Bundle, TableId, TableId)> {
    if !samples.len().is_multiple_of(8) || samples.is_empty() { return Err("invalid twin sample extent".into()); }
    let count = u32::try_from(samples.len() / 8).map_err(|_| "too many twin samples")?;
    let (code, bindings) = shader(ir, "twin", None, 4, ImageFormat::Rgba16Float)?;
    let mut b = Builder { bundle: Bundle::default(), bindings: vec![bindings], wave: 0, baked_inputs: Resources::new() };
    b.bundle.modules.push(ModuleBlob { id: ModuleId(0), target: ShaderTarget::Wgsl,
        hash: *blake3::hash(code.as_bytes()).as_bytes(), bytes: code.into_bytes().into_boxed_slice() });
    b.bundle.params = params;
    let rows = b.table(4, u32::try_from(rows_bytes / 4).map_err(|_| "twin row extent")?, vec![], TableLifetime::External);
    let luts = b.words(1, lut.iter().map(|v| v.to_bits()));
    let kp = b.words(4, kind);
    let input = b.words(8, samples.iter().map(|v| v.to_bits()));
    let output = b.transient(64, count);
    let frame = b.table(frame.len() as u32, 1, frame, TableLifetime::Static);
    b.bundle.resources.frame = Some(FrameData { main: frame, variants: vec![], time_offset: 140 });
    b.compute(0, "twin", DispatchExtent::Fixed([count.div_ceil(64), 1, 1]),
        &Resources::from([((1, 0), rows), ((1, 3), luts), ((1, 2), kp), ((3, 0), input), ((3, 1), output)]))?;
    Ok((b.bundle, rows, output))
}
