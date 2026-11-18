//! Lane 2.2 checks (lean Rust plan §8.2): random typed programs over every operator, loops, selects, calls, fetches,
//! params and inputs emit modules Naga accepts under both `lf` aliases; permuted jobs (shuffled node and decl order,
//! discarded nodes) give byte-identical modules; a raw typo is refused at the author's line.
use loft_core::emit::{Slots, check, module, validate};
use loft_ir::Error;
use loft_ir::error::SceneFault;
use loft_ir::graph::{Graph, GraphFn, Input, Kernel, Loop, Node, Op, Param, Prim, RawFn, Role, Sel, Site, Ty};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[derive(Default, Clone)]
struct B {
    nodes: Vec<Node>,
    args: Vec<u32>,
    lits: Vec<u64>,
    syms: String,
    /// Symbol starts; `graph` appends the end.
    starts: Vec<u32>,
    sites: Vec<Site>,
    node_sites: Vec<u32>,
    fns: Vec<GraphFn>,
    raws: Vec<RawFn>,
    loops: Vec<Loop>,
    kernels: Vec<Kernel>,
    params: Vec<Param>,
    inputs: Vec<Input>,
    /// Layout's slots: per param, per input, per kernel.
    at_params: Vec<u32>,
    at_inputs: Vec<u32>,
    at_kernels: Vec<[u32; 2]>,
    /// Whether a node reads only literals: the fold pass (§1.10) leaves no such operator for emit, and Naga's
    /// constant evaluator refuses one outside its domain (`log` of a negative literal).
    konst: Vec<bool>,
}

impl B {
    fn node(&mut self, op: Op, ty: Ty, args: &[u32]) -> u32 {
        let k = match op {
            Op::Lit(_) => true,
            Op::Prim(_) | Op::Swizzle(_) | Op::Vec => args.iter().all(|&a| self.konst[a as usize]),
            Op::Time
            | Op::Param(_)
            | Op::Input(_)
            | Op::Bind(_)
            | Op::Var(_)
            | Op::Call(_)
            | Op::Fetch { .. }
            | Op::Loop { .. }
            | Op::Grad
            | Op::Raw(_) => false,
        };
        self.konst.push(k);
        self.nodes.push(Node { op, ty, args: self.args.len() as u32, n: args.len() as u32 });
        self.args.extend_from_slice(args);
        self.node_sites.push(0);
        (self.nodes.len() - 1) as u32
    }
    fn lit(&mut self, v: &[f64], ty: Ty) -> u32 {
        let i = self.lits.len() as u32;
        self.lits.extend(v.iter().map(|x| x.to_bits()));
        self.node(Op::Lit(i), ty, &[])
    }
    fn func(&mut self, binders: &[u32], body: u32) -> u32 {
        let ret = self.nodes[body as usize].ty;
        self.fns.push(GraphFn { name: 0, ret, binders: binders.into(), body });
        (self.fns.len() - 1) as u32
    }
    fn sym(&mut self, s: &str) -> u32 {
        self.starts.push(self.syms.len() as u32);
        self.syms.push_str(s);
        (self.starts.len() - 1) as u32
    }
    /// A raw function named `name` with source `text`.
    fn raw(&mut self, name: &str, text: &str, ret: Ty) -> u32 {
        let (name, text) = (self.sym(name), self.sym(text));
        self.raws.push(RawFn { name, ret, text, deps: Box::new([]) });
        (self.raws.len() - 1) as u32
    }
    fn graph(&self) -> Graph {
        let mut sym_at = self.starts.clone();
        sym_at.push(self.syms.len() as u32);
        #[rustfmt::skip]
        let g = Graph { nodes: self.nodes.clone().into(), args: self.args.clone().into(), lits: self.lits.clone().into(),
            rows: Box::new([]), blobs: Box::new([]), syms: self.syms.clone().into(), sym_at: sym_at.into(),
            sites: self.sites.clone().into(), node_sites: self.node_sites.clone().into(), fns: self.fns.clone().into(),
            raws: self.raws.clone().into(), loops: self.loops.clone().into(), kernels: self.kernels.clone().into(),
            params: self.params.clone().into(), inputs: self.inputs.clone().into(), roots: Box::new([]) };
        g
    }
    fn slots(&self) -> Slots {
        Slots {
            params: self.at_params.clone().into(),
            inputs: self.at_inputs.clone().into(),
            kernels: self.at_kernels.clone().into(),
        }
    }
}

const NUM: [Ty; 4] = [Ty::F32, Ty::V2, Ty::V3, Ty::V4];

/// One random operator or vector node over `pool` (values in scope), typed by loft-ir's own rule. None when the draw
/// fails.
fn op(b: &mut B, r: &mut Rng, pool: &[u32]) -> Option<u32> {
    let pick = |r: &mut Rng| pool[r.below(pool.len())];
    if r.below(Prim::ALL.len() + 1) == Prim::ALL.len() {
        // A vector from parts whose widths sum to 2..=4, or one f32 splatted.
        let mut parts = vec![pick(r)];
        let mut w = b.nodes[parts[0] as usize].ty.width()?;
        while w < 2 || (w < 4 && r.below(2) == 0) {
            let x = pick(r);
            let xw = b.nodes[x as usize].ty.width()?;
            if w + xw > 4 {
                break;
            }
            parts.push(x);
            w += xw;
        }
        let ty = match (parts.len(), w) {
            (1, 1) => NUM[1 + r.below(3)],
            (_, w) => *NUM.get(usize::from(w) - 1).filter(|_| w >= 2)?,
        };
        if parts.iter().all(|&a| b.konst[a as usize]) {
            return None;
        }
        return Some(b.node(Op::Vec, ty, &parts));
    }
    let p = Prim::ALL[r.below(Prim::ALL.len())];
    let n = p.arity();
    // Prefer operands of one type, as authored code mostly has.
    let first = pick(r);
    let want = b.nodes[first as usize].ty;
    let mut args = vec![first];
    for _ in 1..n {
        let same: Vec<u32> = pool.iter().copied().filter(|&x| b.nodes[x as usize].ty == want).collect();
        args.push(if r.below(3) > 0 && !same.is_empty() { same[r.below(same.len())] } else { pick(r) });
    }
    if p == Prim::Select {
        let c: Vec<u32> = pool.iter().copied().filter(|&x| b.nodes[x as usize].ty == Ty::Bool).collect();
        args[0] = *c.get(r.below(c.len().max(1)))?;
        let t = b.nodes[args[1] as usize].ty;
        for a in args.iter_mut().skip(2) {
            let s: Vec<u32> = pool.iter().copied().filter(|&x| b.nodes[x as usize].ty == t).collect();
            *a = s[r.below(s.len())];
        }
    }
    if args.iter().all(|&a| b.konst[a as usize]) {
        return None;
    }
    let tys: Vec<Ty> = args.iter().map(|&a| b.nodes[a as usize].ty).collect();
    let ty = p.result(&tys).ok()?;
    Some(b.node(Op::Prim(p), ty, &args))
}

fn grow(b: &mut B, r: &mut Rng, pool: &mut Vec<u32>, n: usize) {
    for _ in 0..n {
        if let Some(x) = op(b, r, pool) {
            pool.push(x);
        }
    }
}

/// A random program: a helper graph function, params, an input, a kernel fetch, a raw function, a loop with a stop,
/// swizzles and operators; the exported function returns a vec4 built from the last values of each type.
fn program(seed: u64) -> (B, u32) {
    let mut r = Rng(seed | 1);
    let mut b = B::default();
    let small = |r: &mut Rng| (r.below(2000) as f64 - 1000.0) / 64.0;
    // helper(x: f32, y: vec3) -> vec3
    let (hx, hy) = (b.node(Op::Bind(900), Ty::F32, &[]), b.node(Op::Bind(901), Ty::V3, &[]));
    let mut hp = vec![hx, hy];
    let l = b.lit(&[0.5], Ty::F32);
    hp.push(l);
    grow(&mut b, &mut r, &mut hp, 40);
    let h3 = *hp.iter().rev().find(|&&x| b.nodes[x as usize].ty == Ty::V3).unwrap_or(&hy);
    let helper = b.func(&[hx, hy], h3);
    // main(p: vec3) -> vec4
    let p = b.node(Op::Bind(1), Ty::V3, &[]);
    let mut pool = vec![p, b.node(Op::Time, Ty::F32, &[])];
    for k in 0..3 {
        let def = b.lits.len() as u32;
        b.lits.extend([small(&mut r), -10.0, 10.0].map(f64::to_bits));
        b.params.push(Param { name: 0, def, min: def + 1, max: def + 2 });
        b.at_params.push(k * 3 + 1);
        pool.push(b.node(Op::Param(k), Ty::F32, &[]));
    }
    let def = b.lits.len() as u32;
    b.lits.extend([1.0f64, 2.0].map(f64::to_bits));
    b.inputs.push(Input { name: 0, ty: Ty::V2, def });
    b.at_inputs.push(2);
    pool.push(b.node(Op::Input(0), Ty::V2, &[]));
    for _ in 0..4 {
        let w = NUM[r.below(4)];
        let v: Vec<f64> = (0..w.width().unwrap_or(1)).map(|_| small(&mut r)).collect();
        pool.push(b.lit(&v, w));
    }
    pool.push(b.lit(&[1.0], Ty::Bool));
    grow(&mut b, &mut r, &mut pool, 120);
    b.kernels.push(Kernel::Table { name: 0, lanes: 3, entries: 1, role: Role::Data });
    b.at_kernels.push([7, 3]);
    let kernel = 0;
    let row = pool[r.below(pool.len())];
    let row = if b.nodes[row as usize].ty == Ty::F32 { row } else { pool[1] };
    pool.push(b.node(Op::Fetch { kernel, lane: 2 }, Ty::V4, &[row]));
    let rs = b.raw("wobble", "fn wobble(x: f32, v: vec3<f32>) -> vec3<f32> {\n  return v * sin(x);\n}", Ty::V3);
    let x = pool[1];
    pool.push(b.node(Op::Raw(rs), Ty::V3, &[x, p]));
    // A march-like loop: state (f32 t, vec3 q), index i, stop when t passes a bound.
    let t0 = b.lit(&[0.0], Ty::F32);
    let (st, sq, si) =
        (b.node(Op::Bind(50), Ty::F32, &[]), b.node(Op::Bind(51), Ty::V3, &[]), b.node(Op::Bind(52), Ty::F32, &[]));
    let mut inner = pool.clone();
    inner.extend([st, sq, si]);
    grow(&mut b, &mut r, &mut inner, 60);
    let nt = *inner.iter().rev().find(|&&x| b.nodes[x as usize].ty == Ty::F32).unwrap_or(&st);
    let nq = *inner.iter().rev().find(|&&x| b.nodes[x as usize].ty == Ty::V3).unwrap_or(&sq);
    let nt = b.node(Op::Prim(Prim::Add), Ty::F32, &[nt, si]);
    let bound = b.lit(&[40.0], Ty::F32);
    let stop = b.node(Op::Prim(Prim::Gt), Ty::Bool, &[st, bound]);
    let count = b.lit(&[24.0], Ty::F32);
    #[rustfmt::skip]
    b.loops.push(Loop { count, init: [t0, p].into(), state: [st, sq].into(), index: si, next: [nt, nq].into(), stop: Some(stop) });
    let lp = (b.loops.len() - 1) as u32;
    pool.push(b.node(Op::Loop { decl: lp, k: 0 }, Ty::F32, &[]));
    pool.push(b.node(Op::Loop { decl: lp, k: 1 }, Ty::V3, &[]));
    let (cx, cy) = (pool[1], *pool.iter().rev().find(|&&x| b.nodes[x as usize].ty == Ty::V3).unwrap_or(&p));
    pool.push(b.node(Op::Call(helper), Ty::V3, &[cx, cy]));
    let v = *pool.iter().rev().find(|&&x| b.nodes[x as usize].ty == Ty::V3).unwrap_or(&p);
    pool.push(b.node(Op::Swizzle(Sel { len: 2, comps: 0b0110 }), Ty::V2, &[v]));
    grow(&mut b, &mut r, &mut pool, 120);
    // Every value reaches the result, so every operator, the loop, the call, the fetch and the raw call are emitted.
    let (zero, one) = (b.lit(&[0.0], Ty::F32), b.lit(&[1.0], Ty::F32));
    let mut acc = pool[1];
    for &x in &pool[2..] {
        let s = match b.nodes[x as usize].ty {
            Ty::F32 => x,
            Ty::Bool => b.node(Op::Prim(Prim::Select), Ty::F32, &[x, one, zero]),
            Ty::V2 | Ty::V3 | Ty::V4 => b.node(Op::Prim(Prim::Length), Ty::F32, &[x]),
        };
        acc = b.node(Op::Prim(Prim::Add), Ty::F32, &[acc, s]);
    }
    let a = *pool.iter().rev().find(|&&x| b.nodes[x as usize].ty == Ty::V3).unwrap_or(&p);
    let out = b.node(Op::Vec, Ty::V4, &[a, acc]);
    let main = b.func(&[p], out);
    (b, main)
}

fn emit_text(b: &B, main: u32) -> String {
    let g = b.graph();
    match module(&g, &[("paint", main)], &b.slots()) {
        Ok(m) => {
            check(&g, &m).unwrap_or_else(|e| panic!("f32 refused: {e}\n{}", m.text));
            validate(&g, &m, true).unwrap_or_else(|e| panic!("f16 refused: {e}\n{}", m.text));
            m.text
        }
        Err(e) => panic!("refused: {e:?}"),
    }
}

#[test]
fn random_programs_validate_both_ways() {
    let (mut ifs, mut loops, mut lets, mut nodes) = (0, 0, 0, 0);
    let mut seen = std::collections::HashSet::new();
    for seed in 1..=1000u64 {
        let (b, main) = program(seed * 0x9e37_79b9);
        let t = emit_text(&b, main);
        if seed == 1 {
            println!("{t}");
        }
        nodes += b.nodes.len();
        seen.extend(b.nodes.iter().filter_map(|n| if let Op::Prim(p) = n.op { Some(p) } else { None }));
        ifs += t.matches("} else {").count();
        loops += t.matches("for (").count();
        lets += t.matches("let v").count();
    }
    println!(
        "1000 programs, {nodes} nodes, {lets} statements, {loops} loops, {ifs} branch-local ifs; all valid as f32 and f16"
    );
    assert_eq!(seen.len(), Prim::ALL.len(), "every operator is emitted");
}

/// A random shuffle of `n` items: `(order, new index of each old one)`.
fn shuffle(r: &mut Rng, n: usize) -> (Vec<usize>, Vec<u32>) {
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        order.swap(i, r.below(i + 1));
    }
    let mut new = vec![0u32; n];
    order.iter().enumerate().for_each(|(i, &o)| new[o] = i as u32);
    (order, new)
}

/// A random topological order of `b` (a Call or Loop after the nodes its decl names), with `dead` discarded nodes
/// mixed in and every decl array shuffled: the same graph as another job would record it.
fn permute(b: &B, main: u32, seed: u64, dead: usize) -> (B, u32) {
    let mut r = Rng(seed | 1);
    let mut src = b.clone();
    let mut pool: Vec<u32> =
        (0..src.nodes.len() as u32).filter(|&n| matches!(src.nodes[n as usize].op, Op::Lit(_) | Op::Time)).collect();
    grow(&mut src, &mut r, &mut pool, dead);
    let n = src.nodes.len();
    let deps = |x: usize| -> Vec<u32> {
        let node = src.nodes[x];
        let mut d = src.args[node.args as usize..(node.args + node.n) as usize].to_vec();
        match node.op {
            Op::Call(f) => {
                let f = &src.fns[f as usize];
                d.extend(f.binders.iter().chain([&f.body]));
            }
            Op::Loop { decl, .. } => d.extend(src.loops[decl as usize].nodes()),
            Op::Lit(_)
            | Op::Time
            | Op::Param(_)
            | Op::Input(_)
            | Op::Bind(_)
            | Op::Var(_)
            | Op::Fetch { .. }
            | Op::Grad
            | Op::Raw(_)
            | Op::Swizzle(_)
            | Op::Vec
            | Op::Prim(_) => {}
        }
        d
    };
    let (mut order, mut placed) = (vec![], vec![false; n]);
    let mut ready: Vec<usize> = vec![];
    let mut waiting: Vec<usize> = (0..n).collect();
    while order.len() < n {
        waiting.retain(|&x| {
            let ok = deps(x).iter().all(|&d| placed[d as usize]);
            if ok {
                ready.push(x);
            }
            !ok
        });
        let x = ready.swap_remove(r.below(ready.len()));
        placed[x] = true;
        order.push(x);
    }
    let mut new_of = vec![0u32; n];
    order.iter().enumerate().for_each(|(i, &o)| new_of[o] = i as u32);
    let m = |x: u32| new_of[x as usize];
    let ms = |xs: &[u32]| -> Box<[u32]> { xs.iter().map(|&x| m(x)).collect() };
    let (fo, fnew) = shuffle(&mut r, src.fns.len());
    let (lo, lnew) = shuffle(&mut r, src.loops.len());
    let (ko, knew) = shuffle(&mut r, src.kernels.len());
    let (po, pnew) = shuffle(&mut r, src.params.len());
    let (io, inew) = shuffle(&mut r, src.inputs.len());
    let mut out = B {
        lits: src.lits.clone(),
        syms: src.syms.clone(),
        starts: src.starts.clone(),
        raws: src.raws.clone(),
        ..B::default()
    };
    for &o in &order {
        let node = src.nodes[o];
        let args: Vec<u32> =
            src.args[node.args as usize..(node.args + node.n) as usize].iter().map(|&a| m(a)).collect();
        let op = match node.op {
            Op::Call(x) => Op::Call(fnew[x as usize]),
            Op::Loop { decl, k } => Op::Loop { decl: lnew[decl as usize], k },
            Op::Fetch { kernel, lane } => Op::Fetch { kernel: knew[kernel as usize], lane },
            Op::Param(x) => Op::Param(pnew[x as usize]),
            Op::Input(x) => Op::Input(inew[x as usize]),
            other @ (Op::Lit(_)
            | Op::Time
            | Op::Bind(_)
            | Op::Var(_)
            | Op::Vec
            | Op::Prim(_)
            | Op::Swizzle(_)
            | Op::Grad
            | Op::Raw(_)) => other,
        };
        out.node(op, node.ty, &args);
    }
    for &o in &fo {
        let f = &src.fns[o];
        out.fns.push(GraphFn { binders: ms(&f.binders), body: m(f.body), ..f.clone() });
    }
    for &o in &lo {
        let l = &src.loops[o];
        #[rustfmt::skip]
        out.loops.push(Loop { count: m(l.count), init: ms(&l.init), state: ms(&l.state), index: m(l.index), next: ms(&l.next), stop: l.stop.map(m) });
    }
    for &o in &ko {
        out.kernels.push(src.kernels[o]);
        out.at_kernels.push(src.at_kernels[o]);
    }
    for &o in &po {
        out.params.push(src.params[o]);
        out.at_params.push(src.at_params[o]);
    }
    for &o in &io {
        out.inputs.push(src.inputs[o]);
        out.at_inputs.push(src.at_inputs[o]);
    }
    (out, fnew[main as usize])
}

#[test]
fn permutations_give_one_module() {
    for seed in 1..=60u64 {
        let (b, main) = program(seed * 7919);
        let want = emit_text(&b, main);
        for k in 0..3 {
            let (p, pm) = permute(&b, main, seed * 31 + k, 40 * k as usize);
            assert_eq!(emit_text(&p, pm), want, "seed {seed} permutation {k}");
        }
    }
}

#[test]
fn raw_typo_is_refused_at_the_authors_line() {
    let mut b = B::default();
    let x = b.node(Op::Bind(0), Ty::F32, &[]);
    let s = b.raw("wobble", "fn wobble(x: f32) -> f32 {\n  let y = x * 2.0\n  return y;\n}", Ty::F32);
    let call = b.node(Op::Raw(s), Ty::F32, &[x]);
    b.sites.push(Site { file: "examples/reef.ts".into(), line: 40, col: 3 });
    b.node_sites[call as usize] = 1;
    let f = b.func(&[x], call);
    let g = b.graph();
    match module(&g, &[("paint", f)], &b.slots()).and_then(|m| check(&g, &m).map(|()| m)) {
        Err(Error::Scene { site: Some(site), fault: SceneFault::Wgsl { line, msg } }) => {
            println!("refused at {site}, raw line {line}: {msg}");
            // Naga points at the token after the missing `;`: `return` on line 3 of the raw source, the author's line 42.
            assert_eq!((&*site.file, site.line, line), ("examples/reef.ts", 42, 3));
        }
        other => panic!("expected Scene{{Wgsl}} at the author's line, got {:?}", other.map(|m| m.text)),
    }
}

#[test]
fn a_value_one_arm_reads_is_computed_under_if() {
    let mut b = B::default();
    let x = b.node(Op::Bind(0), Ty::F32, &[]);
    let z = b.lit(&[0.0], Ty::F32);
    let c = b.node(Op::Prim(Prim::Gt), Ty::Bool, &[x, z]);
    let e = b.node(Op::Prim(Prim::Exp), Ty::F32, &[x]);
    let s = b.node(Op::Prim(Prim::Sin), Ty::F32, &[e]);
    let sel = b.node(Op::Prim(Prim::Select), Ty::F32, &[c, s, z]);
    let f = b.func(&[x], sel);
    let t = emit_text(&b, f);
    println!("{t}");
    assert!(t.contains("if (v0) {\n    let v1 = exp(a0);\n    let v2 = sin(v1);"), "exp and sin move under the if");
}

/// Two raw sources that declare the same function name are refused (Naga: redefinition), never silently merged so
/// that the second author's calls run the first author's body.
#[test]
fn two_raw_sources_with_one_name_are_refused() {
    let mut b = B::default();
    let x = b.node(Op::Bind(0), Ty::F32, &[]);
    let s1 = b.raw("wobble", "fn wobble(x: f32) -> f32 {\n  return x * 2.0;\n}", Ty::F32);
    let s2 = b.raw("wobble", "fn wobble(x: f32) -> f32 {\n  return x * 3.0;\n}", Ty::F32);
    let c1 = b.node(Op::Raw(s1), Ty::F32, &[x]);
    let c2 = b.node(Op::Raw(s2), Ty::F32, &[x]);
    let sum = b.node(Op::Prim(Prim::Add), Ty::F32, &[c1, c2]);
    let f = b.func(&[x], sum);
    let g = b.graph();
    match module(&g, &[("paint", f)], &b.slots()).and_then(|m| check(&g, &m).map(|()| m)) {
        Err(Error::Scene { fault: SceneFault::Wgsl { msg, .. }, .. }) => println!("refused: {msg}"),
        other => panic!("expected Scene{{Wgsl}}, got {:?}", other.map(|m| m.text)),
    }
}
