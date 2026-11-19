use super::{Arena, Expr, Loop, LoopId, Op};
use crate::Error;
use std::collections::HashMap;

/// A fold changes the children of a recorded node. Substitution acts like calling the original constructor again,
/// so an identity can become available when a binder is replaced by a constant.
#[derive(Clone, Copy)]
pub(super) enum Rewrite {
    Fold,
    Substitute,
}

impl Arena {
    pub fn rebuild(&mut self, e: Expr, kids: &[Expr]) -> Result<Expr, Error> {
        self.rebuild_with(e, kids, Rewrite::Substitute)
    }

    pub(super) fn rebuild_with(&mut self, e: Expr, kids: &[Expr], how: Rewrite) -> Result<Expr, Error> {
        if kids.len() != self.children(e).len() {
            return Err(Error::Internal("rebuild: wrong number of children".into()));
        }
        let ty = self[e].ty;
        Ok(match self[e].op.clone() {
            Op::Const(_) | Op::Var(_) | Op::Input(_) | Op::Param(_) => e,
            Op::Row { row, .. } => self.row(row, kids[0])?,
            Op::Fetch { table, lane, .. } => self.fetch(table, kids[0], lane)?,
            Op::Lib { fun, .. } => self.intern_lib(fun, ty, kids),
            Op::Call { prim, .. } => match how {
                Rewrite::Fold => self.call(prim, ty, kids),
                Rewrite::Substitute => self.prim(prim, kids)?,
            },
            Op::Vec(_) => self.intern_vec(ty, kids),
            Op::Get { sel, .. } => self.intern_get(ty, kids[0], &sel),
            Op::Iterate { .. } => {
                return Err(Error::Internal("internal: loops are rebuilt by substitute, not rebuild".into()));
            }
        })
    }

    /// The same expressions with every occurrence of from replaced by to.
    pub fn substitute(&mut self, roots: &[Expr], from: Expr, to: Expr) -> Result<Vec<Expr>, Error> {
        if self[from].ty != self[to].ty {
            return Err(Error::Internal(format!(
                "substitute: cannot replace a {} with a {}",
                self[from].ty.name(),
                self[to].ty.name()
            )));
        }
        self.replace(roots, &HashMap::from([(from, to)]))
    }

    pub fn replace(&mut self, roots: &[Expr], by: &HashMap<Expr, Expr>) -> Result<Vec<Expr>, Error> {
        self.replace_with(roots, by, Rewrite::Substitute)
    }

    fn replace_with(&mut self, roots: &[Expr], by: &HashMap<Expr, Expr>, how: Rewrite) -> Result<Vec<Expr>, Error> {
        let mut done = HashMap::new();
        let mut loops = HashMap::<LoopId, Vec<Expr>>::new();
        for e in self.topo(roots) {
            let made = if let Some(&to) = by.get(&e) {
                if self[to].ty != self[e].ty {
                    return Err(Error::Internal(format!(
                        "replace: cannot replace a {} with a {}",
                        self[e].ty.name(),
                        self[to].ty.name()
                    )));
                }
                to
            } else {
                let kids = self.children(e);
                if kids.iter().all(|k| done[k] == *k) {
                    e
                } else if let Op::Iterate { lo, k } = self[e].op {
                    // Rebuild a loop whose outside changed once, with fresh variables. Shared binders would let
                    // two loops hash-cons their bodies together even though those nodes have different scopes.
                    if let std::collections::hash_map::Entry::Vacant(entry) = loops.entry(lo) {
                        let l = self.loop_at(lo).clone();
                        let init: Vec<_> = l.init.iter().map(|x| done[x]).collect();
                        let made = self.iterate(done[&l.count], &init, |a, state, i| {
                            let body = a.rebase_loop_body(&l, state, i, how)?;
                            let mapped = a.replace_with(&body, by, how)?;
                            Ok((mapped[..l.next.len()].to_vec(), mapped.get(l.next.len()).copied()))
                        })?;
                        entry.insert(made);
                    }
                    loops[&lo][k]
                } else {
                    let mapped: Vec<_> = kids.iter().map(|k| done[k]).collect();
                    self.rebuild_with(e, &mapped, how)?
                }
            };
            done.insert(e, made);
        }
        Ok(roots.iter().map(|r| done[r]).collect())
    }

    pub(super) fn rebase_loop_body(
        &mut self,
        l: &Loop,
        state: &[Expr],
        i: Expr,
        how: Rewrite,
    ) -> Result<Vec<Expr>, Error> {
        let bind = l.state.iter().copied().zip(state.iter().copied()).chain([(l.index, i)]).collect();
        let body: Vec<_> = l.next.iter().copied().chain(l.stop).collect();
        self.replace_with(&body, &bind, how)
    }
}
