use crate::{AuthoredCompute, Prog, arg, bake_tex, emit};
use crate::shader::naga_module;
use crate::source::Graph;
use loft_eval::{eval::RowVal, stage0};
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;
use std::collections::{BTreeMap, HashSet};

fn schedule_name(expr: &Expr) -> Res<&str> {
    match &expr.kind {
        ExprKind::Name(name) => Ok(name),
        _ => Err(Diag::new(Code::E003, expr.span, "expected a declared name", "supply the name without quotes")),
    }
}

pub(crate) fn compute_extent(
    p: &Prog<'_>, types: &mut Vec<Ty>, overrides: &[(String, f64)], value: &Expr,
) -> Res<[u32; 3]> {
    let row = stage0(p, types, value, overrides)?;
    let extent = match row {
        RowVal::U(x) if (1..=65535).contains(&x) => return Ok([x, 1, 1]),
        RowVal::I(x) if (1..=65535).contains(&x) => return Ok([x as u32, 1, 1]),
        RowVal::F(x) => [x, 1.0, 1.0],
        RowVal::V(v) if v.len() == 3 => [v[0], v[1], v[2]],
        _ => [0.0; 3],
    };
    if extent.iter().all(|v| v.fract() == 0.0 && (1.0..=65535.0).contains(v)) {
        return Ok(extent.map(|n| n as u32));
    }
    Err(Diag::new(Code::E602, value.span, "invalid compute extent", "use one or three positive integer workgroup counts"))
}

impl AuthoredCompute {
    pub(super) fn append_to(
        &self, bundle: &mut loft_ir::Bundle, baked_tables: &BTreeMap<&str, loft_ir::TableId>,
    ) -> impl Iterator<Item = (usize, loft_ir::PassDecl)> + use<'_> {
        use loft_ir::{DispatchExtent, ElemFormat, ModuleId, PassRun, TableId, execution::DESCRIPTOR};
        let layouts = bundle.struct_layouts.len() as u32;
        bundle.struct_layouts.extend(self.bundle.struct_layouts.iter().cloned());
        let aliases: BTreeMap<_, _> = baked_tables.iter()
            .filter_map(|(name, id)| Some((*self.tables.get(*name)?, *id)))
            .collect();
        // Execution metadata is rebuilt for the destination graph. Resource IDs
        // still refer to the source declarations, even if the descriptor was interleaved.
        let tables: BTreeMap<_, _> = self.bundle.tables.iter().filter(|table| table.id != DESCRIPTOR)
            .cloned().map(|mut table| {
                let source = table.id;
                if let Some(&id) = aliases.get(&source) { return (source, id); }
                table.id = TableId(bundle.tables.len() as u32);
                if let ElemFormat::Struct(index) = &mut table.format { *index += layouts; }
                let id = table.id;
                bundle.tables.push(table);
                (source, id)
            }).collect();
        bundle.resources.resets.extend(self.bundle.resources.resets.iter().map(|table| tables[table]));
        let modules = bundle.modules.len() as u32;
        bundle.modules.extend(self.bundle.modules.iter().cloned().map(|mut module| {
            module.id.0 += modules;
            module
        }));
        self.positions.iter().copied().zip(self.bundle.passes.iter().cloned().map(move |mut pass| {
            let PassRun::Compute { module, dispatch, .. } = &mut pass.run else { unreachable!("authored compute") };
            *module = ModuleId(module.0 + modules);
            match dispatch {
                DispatchExtent::Fixed(_) => {}
                DispatchExtent::FromRowCount { table, .. } | DispatchExtent::Indirect { buf: table, .. } => {
                    *table = tables[&*table];
                }
            }
            for binding in &mut pass.bindings {
                binding.resource = tables[&binding.resource];
            }
            pass
        }))
    }

    pub(super) fn declare_table<'a>(
        &mut self, p: &Prog<'a>, types: &mut Vec<Ty>, overrides: &[(String, f64)], decl: &'a PassDecl,
    ) -> Res<(&'a str, &'a str)> {
        use loft_ir::{ElemFormat, TableDecl, TableId, TableLifetime};
        let name = decl.args.first().filter(|a| a.name.is_none()).ok_or_else(|| Diag::new(
            Code::E003, decl.verb.span, "table needs a name and capacity", "write table(Name, capacity: n)"))?;
        let name = schedule_name(&name.value)?;
        let mut options = HashSet::new();
        for option in &decl.args[1..] {
            if !option.name.as_ref().is_some_and(|n| matches!(n.name.as_str(), "capacity" | "row") && options.insert(n.name.as_str())) {
                return Err(Diag::new(Code::E003, option.value.span, "invalid table option", "use capacity and optionally row once each"));
            }
        }
        let row = decl.args.iter().find(|a| a.name.as_ref().is_some_and(|n| n.name == "row"))
            .map_or(Ok(name), |a| schedule_name(&a.value))?;
        if !p.structs.contains_key(row) && !p.kinds.contains_key(row) {
            return Err(Diag::new(Code::E011, decl.verb.span, format!("unknown storage row {row}"), "declare a struct to define the row layout"));
        }
        if self.tables.contains_key(name) || p.bakes.contains_key(name) {
            return Err(Diag::new(Code::E011, decl.verb.span, format!("duplicate table {name}"), "give the table a unique name"));
        }
        let expr = arg(decl, "capacity")?;
        let capacity = match stage0(p, types, expr, overrides)? {
            RowVal::U(n) => n,
            RowVal::I(n) if n > 0 => n as u32,
            _ => return Err(Diag::new(Code::E602, expr.span, "invalid table capacity", "use a positive integer row count")),
        };
        let layout = table_layout(p, row, decl.verb.span)?;
        let stride_bytes = layout.stride_bytes;
        if capacity == 0 || stride_bytes.checked_mul(capacity).is_none() {
            return Err(Diag::new(Code::E602, expr.span, "table byte size is out of range", "use a smaller positive capacity"));
        }
        let id = TableId(self.bundle.tables.len() as u32);
        self.tables.insert(name.to_owned(), id);
        let format = ElemFormat::Struct(self.bundle.struct_layouts.len() as u32);
        self.bundle.struct_layouts.push(layout);
        self.bundle.tables.push(TableDecl {
            id, stride_bytes, capacity, format, lifetime: TableLifetime::Static, initial: Box::new([]),
        });
        Ok((name, row))
    }

    pub(super) fn lower(
        &mut self, p: &Prog<'_>, types: &mut Vec<Ty>, src: &Graph, overrides: &[(String, f64)],
        (decl, resources, rows): (&PassDecl, &PassResources, &BTreeMap<&str, &str>),
        features: loft_ir::GpuFeatures,
    ) -> Res<()> {
        use loft_ir::{BindAccess, GpuFeatures};
        let err = |message: String| Diag::new(Code::E520, decl.label.span, message, "match the pass body, tables and declared resource effects");
        if self.truth.contains_key(&decl.label.name) {
            return Err(err(format!("duplicate compute pass {}", decl.label.name)));
        }
        let [Stmt::Expr(Expr { kind: ExprKind::Call { callee, args }, .. })] = resources.body.stmts.as_slice() else {
            return Err(err("a compute pass needs one compute(...) or dispatch(...) call".into()));
        };
        let allowed = self.allowed_resources(p, resources, &err)?;
        let (entry, extent, wgsl, bindings, converted, tier) = match (callee.name.as_str(), args.as_slice()) {
            ("dispatch", [Arg { value: Expr { kind: ExprKind::String(file), .. }, .. },
                          Arg { value: Expr { kind: ExprKind::String(entry), .. }, .. }, extent]) => {
                let extent = compute_extent(p, types, overrides, &extent.value)?;
                let file = serde_json::from_str::<String>(file).map_err(|e| err(format!("invalid WGSL file literal: {e}")))?;
                let entry = serde_json::from_str::<String>(entry).map_err(|e| err(format!("invalid WGSL entry literal: {e}")))?;
                let path = src.source_path(decl.label.span.file).with_file_name(file);
                let file = std::fs::read_to_string(&path).map_err(|e| err(format!("{}: {e}", path.display())))?;
                let (wgsl, bindings) = link_raw_compute(&file, &entry, &allowed, &self.bundle.tables).map_err(err)?;
                (entry, extent, wgsl, bindings, Vec::new(), Determinism::NonDet)
            }
            ("compute", args) => {
                let bad = || err("write compute(entry: kernel_name, extent: n, parameter: value, ...)".into());
                let mut named = BTreeMap::new();
                for arg in args {
                    let name = &arg.name.as_ref().ok_or_else(bad)?.name;
                    if named.insert(name.as_str(), &arg.value).is_some() { return Err(bad()); }
                }
                let entry = schedule_name(named.remove("entry").ok_or_else(bad)?)?;
                let kernel = p.kernels.get(entry).ok_or_else(|| err(format!("unknown kernel {entry}")))?;
                let extent = compute_extent(p, types, overrides, named.remove("extent").ok_or_else(bad)?)?;
                let workgroup = named.remove("workgroup").map_or(Ok([1; 3]), |e| compute_extent(p, types, overrides, e))?;
                if workgroup[0] > 256 || workgroup[1] > 256 || workgroup[2] > 64
                    || workgroup.into_iter().product::<u32>() > 256
                {
                    return Err(err("workgroup size exceeds the portable device limits".into()));
                }
                let scale = named.remove("fixed_scale");
                let float_atomics = match (emit::kernel_uses_float(kernel, types), scale) {
                    (false, None) => emit::FloatAtomics::Native,
                    (false, Some(_)) => return Err(err("fixed_scale requires a float atomic destination".into())),
                    (true, None) => return Err(err("float atomic compute requires a portable fixed_scale".into())),
                    (true, Some(scale)) => {
                        let scale = match stage0(p, types, scale, overrides)? {
                            RowVal::U(n) if n.is_power_of_two() && n <= (1 << 24) => n,
                            RowVal::I(n) if n > 0 && (n as u32).is_power_of_two() && n <= (1 << 24) => n as u32,
                            _ => return Err(err("fixed_scale must be a positive integer power of two up to 2^24".into())),
                        };
                        if features.contains(GpuFeatures::FLOAT32_ATOMICS) { emit::FloatAtomics::Native }
                        else { emit::FloatAtomics::Fixed(scale) }
                    }
                };
                let mut arguments = Vec::new();
                let mut bindings = Vec::new();
                let mut table_args = Vec::new();
                let mut slots = [0; 4];
                let mut used_tables = HashSet::new();
                for param in &kernel.f.params {
                    let name = param.name.name.as_str();
                    let value = named.remove(name).ok_or_else(|| err(format!("kernel {entry} is missing {name}")))?;
                    let ty = param.ty.as_ref().expect("checked kernel parameter");
                    if let TyExpr::Table(row) = ty {
                        let table = schedule_name(value)?;
                        if rows.get(table) != Some(&row.name.as_str()) {
                            return Err(err(format!("{name} needs a table of {}", row.name)));
                        }
                        let &(id, declared) = allowed.get(table).ok_or_else(|| err(format!("{table} is absent from reads and writes")))?;
                        if !used_tables.insert(table) {
                            return Err(err(format!("{table} is bound to more than one kernel parameter")));
                        }
                        if kernel.tier.is_det() && let Some(producer) = self.producers.get(&id) {
                            return Err(Diag::new(Code::E504, value.span, format!("Det kernel {entry} consumes NonDet table {table}"),
                                "use a Det producer or tag the consumer nondet").note(*producer, "NonDet producer"));
                        }
                        let access = if kernel.writes.contains_key(name) { BindAccess::ReadWriteStorage } else { BindAccess::ReadStorage };
                        if access != declared {
                            return Err(err(format!("declared access for {table} disagrees with kernel {entry}")));
                        }
                        let group = access.group();
                        let slot = &mut slots[group as usize];
                        let binding = loft_ir::PassBinding { group, slot: *slot, resource: id, access };
                        *slot += 1;
                        table_args.push((name, row.name.as_str(), binding, self.bundle.tables[id.0 as usize].capacity));
                        bindings.push(binding);
                    } else {
                        arguments.push(loft_eval::kernel_argument(p, types, kernel, value, ty, overrides)?);
                    }
                }
                if !named.is_empty() { return Err(bad()); }
                let (wgsl, converted) = emit::kernel_module(p, types, kernel, &arguments, &table_args, workgroup, float_atomics);
                let module = naga_module(entry, wgsl, src, features | GpuFeatures::SUBGROUPS)?;
                for (_, global) in module.naga.global_variables.iter() {
                    let Some(name) = global.name.as_deref() else { continue; };
                    let Some(name) = name.strip_prefix(bake_tex::SAMPLE_TEXTURE_PREFIX) else { continue; };
                    let &(resource, access) = allowed.get(name).filter(|(_, access)| *access == BindAccess::SampledTex)
                        .ok_or_else(|| err(format!("{name} is absent from reads")))?;
                    let binding = global.binding.as_ref().expect("generated texture binding");
                    bindings.push(loft_ir::PassBinding { group: 3, slot: binding.binding, resource, access });
                    used_tables.insert(name);
                }
                if used_tables.len() != allowed.len() { return Err(bad()); }
                ("main".into(), extent, module.wgsl, bindings, converted, kernel.tier)
            }
            _ => return Err(err("write dispatch(\"module.wgsl\", \"entry\", extent) or compute(entry: kernel, extent: n, ...)".into())),
        };
        self.record_pass(decl, entry, extent, wgsl, bindings, converted, tier, &err)
    }

    fn allowed_resources<'a>(
        &mut self, p: &Prog<'_>, resources: &'a PassResources, err: impl Fn(String) -> Diag,
    ) -> Res<BTreeMap<&'a str, (loft_ir::TableId, loft_ir::BindAccess)>> {
        use loft_ir::BindAccess;
        let mut allowed = BTreeMap::new();
        for (access, names) in [(BindAccess::ReadStorage, &resources.reads), (BindAccess::ReadWriteStorage, &resources.writes)] {
            for name in names {
                if allowed.contains_key(name.name.as_str()) {
                    return Err(err(format!("{} is listed more than once", name.name)));
                }
                if p.bakes.contains_key(name.name.as_str()) {
                    if access != BindAccess::ReadStorage {
                        return Err(err(format!("{} is written by its texture bake", name.name)));
                    }
                    let id = *self.tables.entry(name.name.clone()).or_insert_with(|| {
                        let id = loft_ir::TableId(self.bundle.tables.len() as u32);
                        self.bundle.tables.push(loft_ir::TableDecl {
                            id, format: loft_ir::ElemFormat::F32, lifetime: loft_ir::TableLifetime::External,
                            stride_bytes: 4, capacity: 0, initial: Box::new([]),
                        });
                        id
                    });
                    allowed.insert(name.name.as_str(), (id, BindAccess::SampledTex));
                } else {
                    let table = self.tables.get(&name.name).ok_or_else(|| err(format!("{} is not a declared custom table", name.name)))?;
                    allowed.insert(name.name.as_str(), (*table, access));
                }
            }
        }
        Ok(allowed)
    }

    fn record_pass(
        &mut self, decl: &PassDecl, entry: String, extent: [u32; 3], wgsl: String,
        mut bindings: Vec<loft_ir::PassBinding>, converted: Vec<loft_ir::PassBinding>,
        tier: Determinism, err: impl Fn(String) -> Diag,
    ) -> Res<()> {
        use loft_ir::{BindAccess, DispatchExtent, ModuleBlob, ModuleId, PassId, PassRun, QueueClass, ShaderTarget};
        bindings.sort_by_key(|b| (b.group, b.slot));
        for binding in bindings.iter().filter(|b| b.access == BindAccess::ReadWriteStorage) {
            self.determinism.insert(binding.resource, tier);
            if !tier.is_det() { self.producers.entry(binding.resource).or_insert(decl.label.span); }
        }
        let module = ModuleId(self.bundle.modules.len() as u32);
        self.bundle.modules.push(ModuleBlob {
            id: module, hash: *blake3::hash(wgsl.as_bytes()).as_bytes(), target: ShaderTarget::Wgsl,
            bytes: wgsl.into_bytes().into_boxed_slice(),
        });
        let conversion_groups = converted.iter().map(|b| self.bundle.tables[b.resource.0 as usize].capacity.div_ceil(64))
            .max().map(|groups| [groups.min(65535), 1, 1]);
        let mut append = |entry, extent, bindings: Vec<_>| {
            let wave = self.bundle.passes.len().try_into().map_err(|_| err("too many compute passes".into()))?;
            self.bundle.passes.push(loft_ir::PassDecl {
                id: PassId(u32::from(wave)), wave, queue: QueueClass::Graphics, bindings: bindings.into_boxed_slice(),
                run: PassRun::Compute { module, entry, dispatch: DispatchExtent::Fixed(extent) },
            });
            Ok(())
        };
        if let Some(groups) = conversion_groups {
            append("loft_fixed_prepare".into(), groups, converted.clone())?;
            append(entry, extent, bindings)?;
            append("loft_fixed_finish".into(), groups, converted)?;
        } else {
            append(entry, extent, bindings)?;
        }
        self.tiers.insert(decl.label.name.clone(), tier);
        self.truth.insert(decl.label.name.clone(), loft_ir::Verdict::Unknown { reason: "NoCpuTwin".into() });
        Ok(())
    }
}

fn table_layout(p: &Prog<'_>, row: &str, span: Span) -> Res<loft_ir::StructLayout> {
    let wgsl = emit::table_layout_source(p, row);
    let ir = naga::front::wgsl::parse_str(&wgsl).map_err(|e| Diag::new(Code::E003, span,
        format!("invalid storage row {row}: {}", e.message()), "declare a WGSL host-shareable row"))?;
    let global = ir.global_variables.iter().find(|(_, g)| g.name.as_deref() == Some("loft_table"))
        .expect("emitted table declaration").1;
    let naga::TypeInner::Array { base, stride, .. } = ir.types[global.ty].inner else { unreachable!("emitted array") };
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
        .validate(&ir).map_err(|e| Diag::new(Code::E003, span,
            format!("invalid storage row {row}: {}", e.emit_to_string(&wgsl)), "declare a WGSL host-shareable row"))?;
    let naga::TypeInner::Struct { members, .. } = &ir.types[base].inner else { unreachable!("emitted struct") };
    Ok(loft_ir::StructLayout { stride_bytes: stride, offsets: members.iter().map(|m| m.offset).collect() })
}

fn link_raw_compute(
    source: &str, entry: &str, allowed: &BTreeMap<&str, (loft_ir::TableId, loft_ir::BindAccess)>,
    tables: &[loft_ir::TableDecl],
) -> Result<(String, Vec<loft_ir::PassBinding>), String> {
    use naga::{AddressSpace, ShaderStage, StorageAccess, TypeInner, valid};
    let mut ir = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    let validate = |ir: &naga::Module| valid::Validator::new(valid::ValidationFlags::all(), valid::Capabilities::default() | valid::Capabilities::SUBGROUP)
        .validate(ir).map_err(|e| e.emit_to_string(source));
    let entry_index = ir.entry_points.iter().position(|ep| ep.stage == ShaderStage::Compute && ep.name == entry)
        .ok_or_else(|| format!("WGSL has no compute entry {entry}"))?;
    let info = validate(&ir)?;
    let usage = info.get_entry_point(entry_index);
    let mut mapped = BTreeMap::new();
    for (handle, global) in ir.global_variables.iter() {
        if usage[handle].is_empty() || global.binding.is_none() { continue; }
        let name = global.name.as_deref().ok_or("WGSL global has no resource name")?;
        if global.binding.as_ref().is_some_and(|b| b.group == 0) { continue; }
        let &(resource, access) = allowed.get(name).ok_or_else(|| format!("WGSL binding {name} is absent from reads and writes"))?;
        use loft_ir::BindAccess::*;
        let ty = &ir.types[global.ty].inner;
        let expected = match (global.space, ty) {
            (AddressSpace::Storage { access }, TypeInner::Array { stride, .. })
                if *stride == tables[resource.0 as usize].stride_bytes =>
                if access == StorageAccess::LOAD { ReadStorage } else { ReadWriteStorage },
            (AddressSpace::Handle, TypeInner::Image { class: naga::ImageClass::Sampled { kind: naga::ScalarKind::Float, multi: false }, .. }) =>
                SampledTex,
            _ => return Err(format!("WGSL resource {name} does not match its declared table layout")),
        };
        if access != expected { return Err(format!("WGSL access for {name} disagrees with the pass")); }
        mapped.insert(name.to_owned(), (handle, resource, access));
    }
    if mapped.len() != allowed.len() { return Err("pass declares resources unused by its selected WGSL entry".into()); }
    // Keep the requested entry only. Other entry points may have unrelated interfaces.
    ir.entry_points.retain(|ep| ep.stage == ShaderStage::Compute && ep.name == entry);
    let mut bindings = Vec::new();
    for (_, (handle, resource, access)) in mapped {
        let binding = ir.global_variables[handle].binding.as_ref().expect("linked global");
        let group = access.group();
        if binding.group != u32::from(group) {
            return Err(format!("WGSL binding {} must use group {group} for {access:?}", binding.binding));
        }
        bindings.push(loft_ir::PassBinding { group, slot: binding.binding, resource, access });
    }
    let info = validate(&ir)?;
    let wgsl = naga::back::wgsl::write_string(&ir, &info, naga::back::wgsl::WriterFlags::empty())
        .map_err(|e| e.to_string())?;
    Ok((wgsl, bindings))
}
