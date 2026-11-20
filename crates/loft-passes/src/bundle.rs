//! Convert the existing lowered schedule to the pass-derived bundle ABI.
use crate::{
    Compiled,
    program::{DispatchRows, Draw, Pass, Program},
};
use loft_ir::*;
use naga::{AddressSpace, ShaderStage, StorageAccess, TypeInner, valid};
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, String>;
type Resources<'a> = BTreeMap<&'a str, TableId>;
type Bindings = BTreeMap<(String, ShaderStage), Vec<(String, PassBinding)>>;

struct Builder {
    bundle: Bundle,
    bindings: Vec<Bindings>,
}

impl Builder {
    fn new(program: &Program) -> Result<Self> {
        let mut bundle = Bundle::default();
        let mut bindings = Vec::new();
        for (i, module) in program.modules.iter().enumerate() {
            let mut ir = module.naga.clone();
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
                        _ => return Err(format!("unsupported resource type in {}", module.name)),
                    },
                    _ => {
                        return Err(format!(
                            "unsupported resource address space in {}",
                            module.name
                        ));
                    }
                };
                let group = access.group();
                let slot = &mut slots[group as usize];
                binding.group = u32::from(group);
                binding.binding = *slot;
                let name = global.name.clone().ok_or("unnamed shader resource")?;
                resources.push((
                    handle,
                    name,
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
            .map_err(|e| format!("{}: {e:?}", module.name))?;
            bindings.push(
                ir.entry_points
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        let usage = info.get_entry_point(index);
                        (
                            (entry.name.clone(), entry.stage),
                            resources
                                .iter()
                                .filter(|(handle, _, _)| !usage[*handle].is_empty())
                                .map(|(_, name, binding)| (name.clone(), *binding))
                                .collect(),
                        )
                    })
                    .collect(),
            );
            let code =
                naga::back::wgsl::write_string(&ir, &info, naga::back::wgsl::WriterFlags::empty())
                    .map_err(|e| format!("{}: {e}", module.name))?;
            bundle.modules.push(ModuleBlob {
                id: ModuleId(i as u32),
                hash: *blake3::hash(code.as_bytes()).as_bytes(),
                target: ShaderTarget::Wgsl,
                bytes: code.into_bytes().into_boxed_slice(),
            });
        }
        Ok(Self { bundle, bindings })
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
        resources: &Resources<'_>,
    ) -> Result<()> {
        let mut bindings = BTreeMap::new();
        for (module, name, stage) in entries {
            let decl = self.bindings[module.0 as usize]
                .get(&(name.to_string(), *stage))
                .ok_or_else(|| format!("module {} has no {stage:?} entry {name}", module.0))?;
            for (resource, binding) in decl {
                let mut binding = *binding;
                binding.resource = *resources
                    .get(resource.as_str())
                    .ok_or_else(|| format!("entry {name} needs {resource}"))?;
                if bindings
                    .insert((binding.group, binding.slot), binding)
                    .is_some_and(|b| b != binding)
                {
                    return Err(format!("entry {name} has incompatible bindings"));
                }
            }
        }
        let index = self.bundle.passes.len();
        self.bundle.passes.push(PassDecl {
            id: PassId(index.try_into().map_err(|_| "too many passes")?),
            wave: index.try_into().map_err(|_| "too many pass waves")?,
            queue: QueueClass::Graphics,
            bindings: bindings.into_values().collect(),
            run,
        });
        Ok(())
    }

    fn compute(
        &mut self,
        module: usize,
        name: &str,
        dispatch: DispatchExtent,
        resources: &Resources<'_>,
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
        args: (TableId, u32),
        targets: (TableId, Option<TableId>),
        resources: &Resources<'_>,
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
                    index: None,
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
    if !program.bakes.is_empty() || program.path.is_some() {
        return Err("portable bundles require a typed bake or path resource descriptor".into());
    }
    let mut b = Builder::new(program)?;
    b.bundle.params = parameters(program, compiled)?;
    let mut kinds = Vec::new();
    let mut bases = Vec::new();
    let mut base = 0;
    for (i, kind) in program.kinds.iter().enumerate() {
        let rows = b.words(kind.stride, kind.rows.iter().map(|x| x.to_bits()));
        let params = b.words(4, [i as u32, base, kind.n_rows, 0]);
        bases.push(base);
        base += kind.n_rows;
        b.bundle.rows.insert(KindId(i as u32), rows);
        kinds.push((rows, params));
    }

    let luts = b.words(1, program.luts.iter().map(|x| x.to_bits()));
    for step in &program.dispatches {
        let mut motion = crate::fusion::motion_bundle(program, step, 0);
        let mut resources = Resources::new();
        for table in motion.tables.drain(..) {
            let (name, id) = match table.id.0 {
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
                    ("rows", rows)
                }
                1 => ("loft_advance_args", b.words(6, [0; 6])),
                2 => ("results", b.transient(table.stride_bytes, table.capacity)),
                3 => (
                    "loft_outbox",
                    b.transient(table.stride_bytes, table.capacity),
                ),
                4 => ("luts", luts),
                _ => return Err("unknown fixed-step resource".into()),
            };
            let shader = &program.modules[step.module].naga;
            let original = motion.passes[0]
                .bindings
                .iter()
                .find(|entry| entry.resource == table.id)
                .ok_or("fixed-step resource has no binding")?;
            let symbol = shader
                .global_variables
                .iter()
                .find_map(|(_, global)| {
                    let address = global.binding.as_ref()?;
                    (address.group == u32::from(original.group) && address.binding == original.slot)
                        .then_some(global.name.as_deref())
                        .flatten()
                })
                .ok_or_else(|| format!("missing fixed-step resource {name}"))?;
            resources.insert(symbol, id);
        }
        let module = ModuleId(step.module as u32);
        let rows = *resources.get("rows").ok_or("fixed-step rows are missing")?;
        b.pass(
            crate::fusion::tick_run(step, module, rows),
            &[(module, &step.entry, ShaderStage::Compute)],
            &resources,
        )?;
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
        }),
    );
    let bases = b.words(1, bases);
    let draw_args = b.words(
        4,
        program
            .kinds
            .iter()
            .flat_map(|k| [k.draw.vertices(), 0, 0, 0]),
    );
    let shadow_args = b.words(
        4,
        program
            .kinds
            .iter()
            .flat_map(|k| [k.draw.vertices(), k.n_rows, 0, 0]),
    );
    let shade_args = b.words(3, (0..count).flat_map(|_| [0, 1, 1]));
    let present_args = b.words(4, [3, 1, 0, 0]);
    let visible = b.transient(4, base);
    let all_rows = b.words(1, program.kinds.iter().flat_map(|k| 0..k.n_rows));
    let groups = [size.0.div_ceil(8), size.1.div_ceil(8), 1];
    let tiles = b.transient(4, count * groups[0] * groups[1]);
    let empty = Some([f64::from(u32::MAX), 0., 0., 0.]);
    let far = Some([1., 0., 0., 0.]);
    let vis = b.texture(ImageFormat::Rg32Uint, size, empty);
    let depth = b.texture(ImageFormat::Depth32Float, size, far);
    let side = program.shadow_size.max(1);
    let shadow = b.texture(ImageFormat::Depth32Float, (side, side), far);
    let shadow_vis = b.texture(ImageFormat::Rg32Uint, (side, side), empty);
    let sampler = b.external();
    b.bundle.resources.comparison_samplers.push(sampler);
    let hdr = b.texture(ImageFormat::Rgba16Float, size, None);
    let target = b.external();
    let alternate = program.passes.iter().find_map(|pass| match pass {
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
        let bytes = frame(sun, alternate);
        b.table(bytes.len() as u32, 1, bytes, TableLifetime::Static)
    };
    let main = frame_table(false);
    let alternate = alternate.map(|_| frame_table(true));
    let mut variants = Vec::new();
    b.bundle.resources.resets = vec![draw_args, shade_args];

    for pass in &program.passes {
        match pass {
            Pass::Cull => b.compute(
                program.cull,
                "cull",
                DispatchExtent::FromRowCount {
                    table: instances,
                    workgroup_x: 64,
                },
                &Resources::from([
                    ("insts", instances),
                    ("bases", bases),
                    ("draw_args", draw_args),
                    ("visible", visible),
                ]),
            )?,
            Pass::Classify => b.compute(
                program.classify,
                "classify",
                DispatchExtent::Fixed(groups),
                &Resources::from([
                    ("vis_tex", vis),
                    ("shade_args", shade_args),
                    ("tiles", tiles),
                ]),
            )?,
            Pass::Vis(ids) | Pass::Shadow(ids) | Pass::Shade(ids) => {
                for &id in ids {
                    let k = id as usize;
                    let (rows, kp) = kinds[k];
                    let mut resources =
                        Resources::from([("rows", rows), ("kp", kp), ("luts", luts)]);
                    if matches!(pass, Pass::Vis(_) | Pass::Shadow(_))
                        && matches!(program.kinds[k].draw, Draw::Analytic | Draw::Traced)
                    {
                        b.bundle
                            .resources
                            .cull_front
                            .push(PassId(b.bundle.passes.len() as u32));
                    }
                    match pass {
                        Pass::Vis(_) => {
                            resources.insert("visible", visible);
                            b.draw(
                                program.kinds[k].module,
                                ("vs", "fs"),
                                (draw_args, id * 16),
                                (vis, Some(depth)),
                                &resources,
                            )?;
                        }
                        Pass::Shadow(_) => {
                            variants.push((
                                PassId(b.bundle.passes.len() as u32),
                                alternate.ok_or("missing alternate frame")?,
                            ));
                            resources.insert("visible", all_rows);
                            b.draw(
                                program.kinds[k].module,
                                ("vs", "fs"),
                                (shadow_args, id * 16),
                                (shadow_vis, Some(shadow)),
                                &resources,
                            )?;
                        }
                        Pass::Shade(_) => {
                            resources.extend([
                                ("visible", visible),
                                ("vis_tex", vis),
                                ("hdr", hdr),
                                ("tiles", tiles),
                                ("shadow_tex", shadow),
                                ("shadow_smp", sampler),
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
                        Pass::Cull | Pass::Classify | Pass::Present => unreachable!(),
                    }
                }
            }
            Pass::Present => b.draw(
                program.present,
                ("vs_present", "fs_present"),
                (present_args, 0),
                (target, None),
                &Resources::from([("hdr_tex", hdr), ("vis_tex", vis)]),
            )?,
        }
    }
    b.bundle.resources.frame = Some(FrameData {
        main,
        variants,
        time_offset: 140,
    });
    Ok(b.bundle)
}
