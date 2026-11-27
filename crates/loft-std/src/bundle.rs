//! Validate logical resource roles and pass order without a GPU.
use loft_ir::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
pub enum Role {
    Buffer,
    Texture,
    Attachment(TextureDecl),
    Sampler,
    AccelerationStructure,
    HostAttachment,
}

#[derive(Debug)]
pub enum InvalidResources {
    Layout,
    AttachmentCount,
}

pub struct Resources(BTreeMap<TableId, Role>);

impl Resources {
    pub fn new(bundle: &Bundle) -> Result<Self, InvalidResources> {
        let tables: BTreeMap<_, _> = bundle
            .tables
            .iter()
            .map(|table| (table.id, table.lifetime))
            .collect();
        let mut roles = BTreeMap::new();
        for texture in &bundle.resources.textures {
            if tables.get(&texture.table) != Some(&TableLifetime::External)
                || roles
                    .insert(texture.table, Role::Attachment(*texture))
                    .is_some()
            {
                return Err(InvalidResources::Layout);
            }
        }
        for &id in &bundle.resources.comparison_samplers {
            if tables.get(&id) != Some(&TableLifetime::External)
                || roles.insert(id, Role::Sampler).is_some()
            {
                return Err(InvalidResources::Layout);
            }
        }
        let mut host_color = None;
        let mut host_depth = None;
        for pass in passes(&bundle.passes) {
            match &pass.run {
                PassRun::Compute { .. }
                | PassRun::ComputeFusedTicks { .. }
                | PassRun::ComputeNonDetTicks { .. }
                | PassRun::PassLoop { .. } => {}
                PassRun::GenerateMips { texture } => {
                    assign(&tables, &mut roles, *texture, Role::Texture)?;
                }
                PassRun::RenderBundle {
                    draws,
                    color_targets,
                    depth,
                    ..
                } => {
                    for id in color_targets.iter().map(|t| t.table) {
                        attachment(&tables, &mut roles, &mut host_color, id)?;
                    }
                    if let Some(depth) = depth {
                        attachment(&tables, &mut roles, &mut host_depth, depth.table)?;
                    }
                    for id in draws.iter().filter_map(|draw| draw.vertex) {
                        assign(&tables, &mut roles, id, Role::Buffer)?;
                    }
                }
            }
            for binding in &pass.bindings {
                let role = match binding.access {
                    BindAccess::ReadStorage | BindAccess::ReadWriteStorage => Role::Buffer,
                    BindAccess::SampledTex
                    | BindAccess::StorageTex
                    | BindAccess::BindlessTexArray => Role::Texture,
                    BindAccess::Sampler => Role::Sampler,
                    BindAccess::AccelerationStructure => Role::AccelerationStructure,
                };
                assign(&tables, &mut roles, binding.resource, role)?;
            }
        }
        if host_color.is_some() && host_color == host_depth {
            return Err(InvalidResources::Layout);
        }
        Ok(Self(roles))
    }

    pub fn role(&self, id: TableId) -> Role {
        self.0.get(&id).copied().unwrap_or(Role::Buffer)
    }
}

fn assign(
    tables: &BTreeMap<TableId, TableLifetime>,
    roles: &mut BTreeMap<TableId, Role>,
    id: TableId,
    role: Role,
) -> Result<(), InvalidResources> {
    if !tables.contains_key(&id) {
        return Err(InvalidResources::Layout);
    }
    use std::collections::btree_map::Entry;
    match roles.entry(id) {
        Entry::Vacant(entry)
            if !matches!(role, Role::Sampler) || tables[&id] == TableLifetime::External => {
            entry.insert(role);
        }
        Entry::Occupied(entry)
            if matches!(
                (entry.get(), role),
                (Role::Buffer, Role::Buffer)
                    | (Role::Texture | Role::Attachment(_), Role::Texture)
                    | (Role::Sampler, Role::Sampler)
                    | (Role::AccelerationStructure, Role::AccelerationStructure)
            ) => {}
        _ => return Err(InvalidResources::Layout),
    }
    Ok(())
}

fn attachment(
    tables: &BTreeMap<TableId, TableLifetime>,
    roles: &mut BTreeMap<TableId, Role>,
    host: &mut Option<TableId>,
    id: TableId,
) -> Result<(), InvalidResources> {
    match roles.get(&id) {
        Some(Role::Attachment(_)) => Ok(()),
        None | Some(Role::HostAttachment)
            if tables
                .get(&id)
                .is_none_or(|lifetime| *lifetime == TableLifetime::External) =>
        {
            if *host.get_or_insert(id) != id {
                return Err(InvalidResources::AttachmentCount);
            }
            roles.insert(id, Role::HostAttachment);
            Ok(())
        }
        _ => Err(InvalidResources::Layout),
    }
}

pub fn passes(decls: &[PassDecl]) -> impl Iterator<Item = &PassDecl> {
    let mut stack = vec![decls.iter()];
    std::iter::from_fn(move || {
        loop {
            if let Some(pass) = stack.last_mut()?.next() {
                if let PassRun::PassLoop { body, .. } = &pass.run {
                    stack.push(body.iter());
                }
                return Some(pass);
            }
            stack.pop();
        }
    })
}

pub fn distinct_passes(decls: &[PassDecl]) -> bool {
    let mut ids = BTreeSet::new();
    passes(decls).all(|pass| ids.insert(pass.id))
}

pub fn children(run: &PassRun) -> &[PassDecl] {
    match run {
        PassRun::PassLoop { body, .. } => body,
        PassRun::Compute { .. }
        | PassRun::ComputeFusedTicks { .. }
        | PassRun::ComputeNonDetTicks { .. }
        | PassRun::RenderBundle { .. }
        | PassRun::GenerateMips { .. } => &[],
    }
}
