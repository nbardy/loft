use crate::artefact::{Detail, Draw, TablePlace};

/// The highest slot written, plus one; the unused tail of the uniform never needs to be written.
pub(super) fn slots_used(draws: &[Draw]) -> usize {
    draws.iter().fold(0, |n, d| match d.detail {
        Detail::Fixed => n,
        Detail::Grid { slot, .. } => n.max(slot + d.instances),
        Detail::Projected { slot, .. } => n.max(slot + 2),
    })
}

pub(super) struct GridBound<'a> {
    pub slot: usize,
    pub bound: &'a TablePlace,
    pub instances: usize,
}

/// The grids among these draws: each grid slot once, in first-seen order, with the last draw's values.
pub(super) fn grid_bounds(draws: &[Draw]) -> Vec<GridBound<'_>> {
    let mut seen: Vec<GridBound<'_>> = Vec::new();
    for d in draws {
        match &d.detail {
            Detail::Fixed | Detail::Projected { .. } => {}
            Detail::Grid { slot, bound, .. } => {
                let item = GridBound { slot: *slot, bound, instances: d.instances };
                if let Some(first) = seen.iter_mut().find(|g| g.slot == *slot) {
                    *first = item;
                } else {
                    seen.push(item);
                }
            }
        }
    }
    seen
}
