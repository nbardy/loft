use super::{Arena, Expr, InputSpec, LibId, Op, ParamSpec, RowId, TableId, Ty};
use crate::Error;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct Stamps {
    seen: Vec<u32>,
    walk: u32,
}

impl Stamps {
    fn start(&mut self, len: usize) {
        if self.seen.len() < len {
            self.seen.resize(len.max(2 * self.seen.len()).max(4096), 0);
        }
        self.walk = self.walk.wrapping_add(1);
        if self.walk == 0 {
            self.seen.fill(0);
            self.walk = 1;
        }
    }

    fn enter(&mut self, e: Expr) -> bool {
        let stamp = &mut self.seen[e.0];
        if *stamp == self.walk {
            return false;
        }
        *stamp = self.walk;
        true
    }
}

pub struct Leaves<'a> {
    pub vars: Vec<(&'a str, Ty)>,
    pub inputs: Vec<&'a InputSpec>,
    pub params: Vec<&'a ParamSpec>,
    pub rows: Vec<RowId>,
    pub tables: Vec<TableId>,
    pub libs: Vec<LibId>,
}

impl Arena {
    /// Every node reachable from roots, dependencies first.
    pub fn topo(&self, roots: &[Expr]) -> Vec<Expr> {
        fn visit(arena: &Arena, e: Expr, seen: &mut Stamps, out: &mut Vec<Expr>) {
            if seen.enter(e) {
                for &c in arena.children(e) {
                    visit(arena, c, seen, out);
                }
                out.push(e);
            }
        }
        // One stamp array for the many walks a compile makes. Neither walk calls another while holding it.
        let mut seen = self.stamps.borrow_mut();
        seen.start(self.nodes.len());
        let mut out = Vec::new();
        for &e in roots {
            visit(self, e, &mut seen, &mut out);
        }
        out
    }

    /// Every node including loop bodies, excluding the loops' own variables.
    pub fn everything(&self, roots: &[Expr]) -> Vec<Expr> {
        fn add(arena: &Arena, list: &[Expr], seen: &mut Stamps, out: &mut Vec<Expr>) {
            for &e in list {
                if seen.enter(e) {
                    out.push(e);
                    if let Op::Iterate { lo, .. } = arena[e].op {
                        add(arena, &arena.loop_at(lo).inner, seen, out);
                    }
                }
            }
        }
        let list = self.topo(roots);
        let mut seen = self.stamps.borrow_mut();
        seen.start(self.nodes.len());
        let mut out = Vec::new();
        add(self, &list, &mut seen, &mut out);
        out
    }

    /// Inner nodes depend on vars. Outer nodes are their other immediate children, followed by independent roots.
    pub fn split_on(&self, roots: &[Expr], vars: &[Expr]) -> (Vec<Expr>, Vec<Expr>) {
        let own: HashSet<_> = vars.iter().copied().collect();
        let mut dependent = HashSet::new();
        let mut inner = Vec::new();
        for e in self.topo(roots) {
            if !own.contains(&e) && self.children(e).iter().any(|c| own.contains(c) || dependent.contains(c)) {
                dependent.insert(e);
                inner.push(e);
            }
        }
        let mut seen = HashSet::new();
        let outer = inner
            .iter()
            .flat_map(|&e| self.children(e))
            .chain(roots)
            .copied()
            .filter(|e| !own.contains(e) && !dependent.contains(e) && seen.insert(*e))
            .collect();
        (inner, outer)
    }

    /// The leaves a set of expressions depends on, in encounter order.
    pub fn leaves(&self, roots: &[Expr]) -> Result<Leaves<'_>, Error> {
        fn put<K: std::hash::Hash + Eq, V>(order: &mut Vec<V>, indices: &mut HashMap<K, usize>, k: K, v: V) {
            if let Some(&i) = indices.get(&k) {
                order[i] = v;
            } else {
                indices.insert(k, order.len());
                order.push(v);
            }
        }
        let mut vars = Vec::new();
        let mut inputs = Vec::new();
        let mut params = Vec::new();
        let mut rows = Vec::new();
        let mut tables = Vec::new();
        let (mut by_var, mut by_input, mut by_param) = (HashMap::new(), HashMap::new(), HashMap::new());
        let (mut by_row, mut by_table) = (HashSet::new(), HashSet::new());
        let nodes = self.everything(roots);
        for &e in &nodes {
            let n = &self[e];
            match &n.op {
                Op::Var(name) => put(&mut vars, &mut by_var, name.as_str(), (name.as_str(), n.ty)),
                Op::Input(spec) => put(&mut inputs, &mut by_input, spec.name.as_str(), spec),
                Op::Param(spec) => put(&mut params, &mut by_param, spec.name.as_str(), spec),
                Op::Row { row, .. } => {
                    if by_row.insert(*row) {
                        rows.push(*row);
                    }
                }
                Op::Fetch { table, .. } => {
                    if by_table.insert(*table) {
                        tables.push(*table);
                    }
                }
                Op::Const(_) | Op::Call { .. } | Op::Vec(_) | Op::Get { .. } | Op::Lib { .. } | Op::Iterate { .. } => {}
            }
        }
        Ok(Leaves { vars, inputs, params, rows, tables, libs: self.libs_in(&nodes)? })
    }

    /// The library functions used, each after the functions it depends on.
    pub fn libs_of(&self, roots: &[Expr]) -> Result<Vec<LibId>, Error> {
        self.libs_in(&self.everything(roots))
    }

    /// Also used by leaves so it only walks its roots once.
    pub fn libs_in(&self, nodes: &[Expr]) -> Result<Vec<LibId>, Error> {
        fn add<'a>(
            arena: &'a Arena,
            id: LibId,
            seen: &mut HashSet<LibId>,
            by_name: &mut HashMap<&'a str, LibId>,
            out: &mut Vec<LibId>,
        ) -> Result<(), Error> {
            if !seen.insert(id) {
                return Ok(());
            }
            let fun = arena.lib_fn(id);
            // A definition reached only through deps never passes through lib(), so check for name collisions here.
            if by_name.insert(&fun.name, id).is_some_and(|other| other != id) {
                return Err(Error::Internal(format!(
                    "library function \"{}\" is defined by two different objects",
                    fun.name
                )));
            }
            for &dep in &fun.deps {
                add(arena, dep, seen, by_name, out)?;
            }
            out.push(id);
            Ok(())
        }
        let (mut out, mut seen, mut by_name) = (Vec::new(), HashSet::new(), HashMap::new());
        for &e in nodes {
            if let Op::Lib { fun, .. } = self[e].op {
                add(self, fun, &mut seen, &mut by_name, &mut out)?;
            }
        }
        Ok(out)
    }
}
