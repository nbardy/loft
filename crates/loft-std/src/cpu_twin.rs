//! Optional synchronous CPU execution of deterministic fixed-step passes.
mod compute;

use crate::{error::Result, stepping::{Error, Fused, History, arguments, chunks}};
use loft_abi::{AdvanceArgs, advance};
use loft_ir::*;
use std::{collections::BTreeMap, ops::Range};

type Buffers = BTreeMap<(u32, u32), Vec<u8>>;

fn key(b: PassBinding) -> (u32, u32) {
    (u32::from(b.group), b.slot)
}

/// Evaluate a fixed-step pass without a device. The caller owns the packed table bytes.
pub fn step_bundle_cpu(
    bundle: &Bundle,
    pass: PassId,
    tables: &mut BTreeMap<TableId, Vec<u8>>,
    inst: InstanceId,
    rows: Range<u32>,
    ticks: AdvanceArgs,
) -> Result<()> {
    let decl = bundle
        .passes
        .iter()
        .find(|p| p.id == pass)
        .ok_or(Error::UnknownResource)?;
    let module = cpu_tick_module(&decl.run)?;
    let blob = bundle
        .modules
        .iter()
        .find(|b| b.id == module)
        .ok_or(Error::UnknownResource)?;
    let code = std::str::from_utf8(&blob.bytes).map_err(|_| Error::AbiMismatch)?;
    // CPU validation retains its empty capability set and existing diagnostic text.
    let (ir, _) = loft_format::wgsl::validate(code, &mut naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    ))
    .map_err(|e| Error::InvalidShader(match &e {
        loft_format::wgsl::Error::Parse(_) => e.emit_to_string(code),
        loft_format::wgsl::Error::Validate(_) => e.to_string(),
    }))?;
    let f = Fused::reflect(&ir, decl)?;
    let mut buffers = BTreeMap::from([(
        (0, 2),
        crate::params::Params::new(&bundle.params, u32::MAX.into())?
            .bytes()
            .to_vec(),
    )]);
    for b in decl.bindings.iter().filter(|b| **b != f.args) {
        buffers.insert(
            key(*b),
            tables
                .get(&b.resource)
                .ok_or(Error::UnknownResource)?
                .clone(),
        );
    }
    let decl_rows = bundle
        .tables
        .iter()
        .find(|t| t.id == f.rows)
        .ok_or(Error::UnknownResource)?;
    if rows.start > rows.end
        || rows.end > decl_rows.capacity
        || advance(ticks.from_tick, ticks.to_tick, ticks.keep_stride).is_none()
        || ticks.to_tick - ticks.from_tick > f.max_ticks
    {
        return Err(Error::Capacity);
    }
    run(&f, inst, rows, ticks, &mut buffers)?;
    for binding in decl
        .bindings
        .iter()
        .filter(|b| b.access == BindAccess::ReadWriteStorage)
    {
        tables.insert(binding.resource, buffers.remove(&key(*binding)).unwrap());
    }
    Ok(())
}

fn cpu_tick_module(run: &PassRun) -> Result<ModuleId> {
    match run {
        PassRun::ComputeFusedTicks { module, .. } => Ok(*module),
        PassRun::ComputeNonDetTicks { .. } => Err(Error::Unsupported("NonDet ticks require GPU execution")),
        PassRun::Compute { .. } | PassRun::RenderBundle { .. } | PassRun::PassLoop { .. }
        | PassRun::GenerateMips { .. } => Err(Error::AbiMismatch),
    }
}

fn run(
    f: &Fused,
    id: InstanceId,
    rows: Range<u32>,
    ticks: AdvanceArgs,
    buffers: &mut Buffers,
) -> Result<()> {
    buffers.insert(key(f.args), arguments(id, &rows, ticks).into());
    compute::dispatch(&f.module, &f.entry, rows.end - rows.start, buffers).map_err(|err| match err {
        compute::Error::Unsupported => {
            Error::Unsupported("operation has no deterministic CPU implementation")
        }
        compute::Error::Capacity => Error::Capacity,
        compute::Error::MissingEntry => Error::UnknownResource,
    })
}

impl History {
    pub fn step_live(&mut self, f: &Fused, decl: &PassDecl, active: u32, stride_bytes: u32,
        rows: Range<u32>, ticks: u32, inst: InstanceId, outbox: Option<TableId>, params: &[u8],
        mut read: impl FnMut(TableId) -> Result<Vec<u8>>, mut write: impl FnMut(TableId, &[u8]) -> Result<()>) -> Result<&[u8]> {
        // Live stepping reaches the same CPU interpreter as offline bundle stepping.
        cpu_tick_module(&decl.run)?;
        let pass = decl.id;
        if rows.start > rows.end || rows.end > active {
            return Err(Error::Capacity);
        }
        if rows.is_empty() {
            self.returned.clear();
            return Ok(&self.returned);
        }
        let mut buffers = BTreeMap::new();
        for b in decl.bindings.iter().filter(|b| **b != f.args) {
            let mut bytes = read(b.resource)?;
            if outbox == Some(b.resource) {
                let n = crate::events::capacity(bytes.len() as u64)?;
                bytes[4..8].copy_from_slice(&n.to_le_bytes());
            }
            buffers.insert(key(*b), bytes);
        }
        buffers.insert((0, 2), params.to_vec());
        self.step(f, pass, active, rows.clone(), ticks, inst, &mut buffers)?;
        let state_binding = decl
            .bindings
            .iter()
            .find(|b| b.resource == f.rows)
            .ok_or(Error::AbiMismatch)?;
        let stride = stride_bytes as usize;
        let byte_range = rows.start as usize * stride..rows.end as usize * stride;
        for binding in decl
            .bindings
            .iter()
            .filter(|b| b.access == BindAccess::ReadWriteStorage)
        {
            write(binding.resource, &buffers[&key(*binding)])?;
        }
        self.returned = buffers[&key(*state_binding)][byte_range].to_vec();
        Ok(&self.returned)
    }

    fn step(
        &mut self,
        f: &Fused,
        pass: PassId,
        active: u32,
        rows: Range<u32>,
        ticks: u32,
        inst: InstanceId,
        buffers: &mut Buffers,
    ) -> Result<()> {
        let cursors = self.seek(pass, active);
        let mut updated = cursors.to_vec();
        let mut first = rows.start;
        while first < rows.end {
            let start = cursors[first as usize];
            let end = start.checked_add(ticks).ok_or(Error::Capacity)?;
            let last = (first + 1..rows.end)
                .find(|&i| cursors[i as usize] != start)
                .unwrap_or(rows.end);
            for args in chunks(start, end, f.max_ticks, 0) {
                run(f, inst, first..last, args, buffers)?;
            }
            updated[first as usize..last as usize].fill(end);
            first = last;
        }
        cursors.copy_from_slice(&updated);
        Ok(())
    }
}
