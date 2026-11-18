//! The schedule (plan §1.10): the order the host dispatches kernels in. Load kernels run once, then per frame the
//! stepped kernels the time needs, then the frame's kernels by level. A kernel's level is one more than the deepest
//! kernel of its phase whose resource it reads (TS: a table's level, src/compile/tables.ts). Map, Scan and
//! frame-values kernels of one phase and level are one dispatch, as TS's pose stages are; a raw pass and a stepped
//! kernel are a dispatch each.
use crate::kernel::{Decl, Fill, Job, Kernel, Plan};
use loft_ir::Error;
use loft_ir::bundle::When;
use loft_ir::error::SceneFault;

fn phase(w: &When) -> (u8, &'static str) {
    match w {
        When::Load => (0, "once at load"),
        When::Steps { .. } => (1, "per step"),
        When::Frame => (2, "per frame"),
    }
}

fn refuse(what: String) -> Error {
    Error::Scene { site: None, fault: SceneFault::Unsupported { what: what.into() } }
}

/// Each kernel's level in its phase. A read reaches into an earlier phase freely, never a later one; a writer of the
/// kernel's own resource (a stepped kernel's previous state, another run of its table) is not a dependency.
pub fn levels(ks: &[Kernel], n_res: usize) -> Result<Vec<u32>, Error> {
    let mut writers = vec![Vec::new(); n_res];
    for (k, x) in ks.iter().enumerate() {
        writers[x.writes as usize].push(k);
    }
    let mut level: Vec<Option<u32>> = vec![None; ks.len()];
    let mut open = vec![false; ks.len()];
    fn visit(
        k: usize,
        ks: &[Kernel],
        w: &[Vec<usize>],
        level: &mut [Option<u32>],
        open: &mut [bool],
    ) -> Result<u32, Error> {
        if let Some(l) = level[k] {
            return Ok(l);
        }
        if open[k] {
            return Err(refuse(format!(
                "resource {} is filled from itself through others in one phase: break the cycle with a stepped kernel",
                ks[k].writes
            )));
        }
        open[k] = true;
        let mut l = 0;
        for &r in ks[k].reads.iter().filter(|&&r| r != ks[k].writes) {
            for &j in &w[r as usize] {
                let ((pj, wj), (pk, wk)) = (phase(&ks[j].when), phase(&ks[k].when));
                match pj.cmp(&pk) {
                    std::cmp::Ordering::Less => {}
                    std::cmp::Ordering::Equal => l = l.max(visit(j, ks, w, level, open)? + 1),
                    std::cmp::Ordering::Greater => {
                        return Err(refuse(format!(
                            "a fill run {wk} reads resource {r}, which is filled {wj}: read it from a fill run {wj} or later"
                        )));
                    }
                }
            }
        }
        open[k] = false;
        level[k] = Some(l);
        Ok(l)
    }
    (0..ks.len()).map(|k| visit(k, ks, &writers, &mut level, &mut open)).collect()
}

/// The dispatches, in order: each a list of kernels (indices into `plan.kernels`) the host runs as one.
pub fn schedule(plan: &Plan, decls: &[Decl]) -> Result<Vec<Box<[u32]>>, Error> {
    let level = levels(&plan.kernels, plan.resources.len())?;
    let alone = |k: &Kernel| match (&k.job, &k.when) {
        (_, When::Steps { .. }) => true,
        (Job::Decl(d), When::Load | When::Frame) => match decls[*d as usize].fill {
            Fill::Raw { .. } => true,
            Fill::Map { .. } | Fill::Scan { .. } => false,
        },
        (Job::Lifted(_), When::Load | When::Frame) => false,
    };
    let key =
        |k: usize| (phase(&plan.kernels[k].when).0, level[k], if alone(&plan.kernels[k]) { k } else { usize::MAX });
    let mut order: Vec<usize> = (0..plan.kernels.len()).collect();
    order.sort_by_key(|&k| key(k));
    Ok(order.chunk_by(|&a, &b| key(a) == key(b)).map(|s| s.iter().map(|&k| k as u32).collect()).collect())
}
