//! CPU execution of deterministic fixed-step passes.
mod compute;

use loft_abi::{AdvanceArgs, advance};
use loft_ir::*;
use loft_std::{error::Result, params::Params, stepping::{Error, Fused, arguments}};
use std::{collections::BTreeMap, ops::Range};

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
    let (module, entry) = match &decl.run {
        PassRun::ComputeFusedTicks { module, entry, .. } => (*module, entry),
        PassRun::ComputeNonDetTicks { .. } => {
            return Err(Error::Unsupported("NonDet ticks require GPU execution"));
        }
        PassRun::Compute { .. }
        | PassRun::RenderBundle { .. }
        | PassRun::PassLoop { .. }
        | PassRun::GenerateMips { .. } => return Err(Error::AbiMismatch),
    };
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
        Params::new(&bundle.params, u32::MAX.into())?
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
    buffers.insert(key(f.args), arguments(inst, &rows, ticks).into());
    compute::dispatch(&ir, entry, rows.end - rows.start, &mut buffers).map_err(|err| match err {
        compute::Error::Unsupported => {
            Error::Unsupported("operation has no deterministic CPU implementation")
        }
        compute::Error::Capacity => Error::Capacity,
        compute::Error::MissingEntry => Error::UnknownResource,
    })?;
    for binding in decl
        .bindings
        .iter()
        .filter(|b| b.access == BindAccess::ReadWriteStorage)
    {
        tables.insert(binding.resource, buffers.remove(&key(*binding)).unwrap());
    }
    Ok(())
}
