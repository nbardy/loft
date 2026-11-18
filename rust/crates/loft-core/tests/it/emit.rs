//! Lane 2.2 checks (lean Rust plan §8.2): random typed programs over every operator, loops, selects, calls, fetches,
//! params and inputs emit modules Naga accepts under both `lf` aliases; permuted jobs (shuffled node and decl order,
//! discarded nodes) give byte-identical modules; a raw typo is refused at the author's line.
use loft_core::emit::{module, validate};
use loft_ir::Error;
use loft_ir::error::SceneFault;
use loft_ir::graph::{Decl, Graph, Leaf, Node, Op, Prim, Sel, Site, Tag, Ty};

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
    decls: Vec<Decl>,
    roots: Vec<u32>,
    syms: String,
    ends: Vec<u32>,
    sites: Vec<Site>,
    node_sites: Vec<u32>,
    at: Vec<[u32; 2]>,
    /// Whether a node reads only literals: the fold pass (§1.10) leaves no such operator for emit, and Naga's
    /// constant evaluator refuses one outside its domain (`log` of a negative literal).
    konst: Vec<bool>,
}

impl B {
    fn node(&mut self, op: Op, ty: Ty, args: &[u32]) -> u32 {
        let k = match op {
            Op::Lit(_) => true,
            Op::Prim(_) | Op::Swizzle(_) => args.iter().all(|&a| self.konst[a as usize]),
            Op::Leaf(_) | Op::Call(_) | Op::Fetch { .. } | Op::Loop { .. } | Op::Grad | Op::Raw(_) => false,
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
    fn decl(&mut self, tag: Tag, arity: u32, roots: &[u32], at: [u32; 2]) -> u32 {
        self.decls.push(Decl { tag, arity, name: None, roots: self.roots.len() as u32, n: roots.len() as u32 });
        self.roots.extend_from_slice(roots);
        self.at.push(at);
        (self.decls.len() - 1) as u32
    }
    fn sym(&mut self, s: &str) -> u32 {
        self.syms.push_str(s);
        self.ends.push(self.syms.len() as u32);
        (self.ends.len() - 1) as u32
    }
    fn graph(&self) -> Graph {
        #[rustfmt::skip]
        let g = Graph { nodes: self.nodes.clone().into(), args: self.args.clone().into(), lits: self.lits.clone().into(),
            rows: Box::new([]), blobs: Box::new([]), syms: self.syms.clone().into(), sym_ends: self.ends.clone().into(),
            sites: self.sites.clone().into(), node_sites: self.node_sites.clone().into(), decls: self.decls.clone().into(),
            roots: self.roots.clone().into() };
        g
    }
}

const NUM: [Ty; 4] = [Ty::F32, Ty::V2, Ty::V3, Ty::V4];

/// One random operator node over `pool` (values in scope), typed by loft-ir's own rule. None when the draw fails.
fn op(b: &mut B, r: &mut Rng, pool: &[u32]) -> Option<u32> {
    let p = Prim::ALL[r.below(Prim::ALL.len())];
    let pick = |r: &mut Rng| pool[r.below(pool.len())];
    // Vec takes 1..=4 parts and Pick an index and 1+ values; every other operator one fixed count.
    let n = (1..=5).filter(|&k| p.takes(k)).nth(r.below(4)).or((1..=3).find(|&k| p.takes(k)))?;
    // Prefer operands of one type, as authored code mostly has.
    let first = pick(r);
    let want = b.nodes[first as usize].ty;
    let mut args = vec![first];
    for _ in 1..n {
        let same: Vec<u32> = pool.iter().copied().filter(|&x| b.nodes[x as usize].ty == want).collect();
        args.push(if r.below(3) > 0 && !same.is_empty() { same[r.below(same.len())] } else { pick(r) });
    }
    if p == Prim::Select || p == Prim::Pick {
        let lead = if p == Prim::Select { Ty::Bool } else { Ty::F32 };
        let c: Vec<u32> = pool.iter().copied().filter(|&x| b.nodes[x as usize].ty == lead).collect();
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
    let recorded = NUM[1 + r.below(3)];
    let tys: Vec<Ty> = args.iter().map(|&a| b.nodes[a as usize].ty).collect();
    let ty = p.result(&tys, recorded).ok()?;
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
    let (hx, hy) = (b.node(Op::Leaf(Leaf::Bind(900)), Ty::F32, &[]), b.node(Op::Leaf(Leaf::Bind(901)), Ty::V3, &[]));
    let mut hp = vec![hx, hy];
    let l = b.lit(&[0.5], Ty::F32);
    hp.push(l);
    grow(&mut b, &mut r, &mut hp, 40);
    let h3 = *hp.iter().rev().find(|&&x| b.nodes[x as usize].ty == Ty::V3).unwrap_or(&hy);
    let helper = b.decl(Tag::Fn, 2, &[hx, hy, h3], [0, 0]);
    // main(p: vec3) -> vec4
    let p = b.node(Op::Leaf(Leaf::Bind(1)), Ty::V3, &[]);
    let mut pool = vec![p, b.node(Op::Leaf(Leaf::Time), Ty::F32, &[])];
    for k in 0..3 {
        let (d, lo, hi) = (b.lit(&[small(&mut r)], Ty::F32), b.lit(&[-10.0], Ty::F32), b.lit(&[10.0], Ty::F32));
        let pd = b.decl(Tag::Param, 0, &[d, lo, hi], [k * 3 + 1, 0]);
        pool.push(b.node(Op::Leaf(Leaf::Param(pd)), Ty::F32, &[]));
    }
    let def = b.lit(&[1.0, 2.0], Ty::V2);
    let input = b.decl(Tag::Input, 0, &[def], [2, 0]);
    pool.push(b.node(Op::Leaf(Leaf::Input(input)), Ty::V2, &[]));
    for _ in 0..4 {
        let w = NUM[r.below(4)];
        let v: Vec<f64> = (0..w.width().unwrap_or(1)).map(|_| small(&mut r)).collect();
        pool.push(b.lit(&v, w));
    }
    pool.push(b.lit(&[1.0], Ty::Bool));
    grow(&mut b, &mut r, &mut pool, 120);
    let kernel = b.decl(Tag::Kernel, 0, &[], [7, 3]);
    let row = pool[r.below(pool.len())];
    let row = if b.nodes[row as usize].ty == Ty::F32 { row } else { pool[1] };
    pool.push(b.node(Op::Fetch { kernel, lane: 2 }, Ty::V4, &[row]));
    let rs = b.sym("fn wobble(x: f32, v: vec3<f32>) -> vec3<f32> {\n  return v * sin(x);\n}");
    let x = pool[1];
    pool.push(b.node(Op::Raw(rs), Ty::V3, &[x, p]));
    // A march-like loop: state (f32 t, vec3 q), index i, stop when t passes a bound.
    let t0 = b.lit(&[0.0], Ty::F32);
    let (st, sq, si) = (
        b.node(Op::Leaf(Leaf::Bind(50)), Ty::F32, &[]),
        b.node(Op::Leaf(Leaf::Bind(51)), Ty::V3, &[]),
        b.node(Op::Leaf(Leaf::Bind(52)), Ty::F32, &[]),
    );
    let mut inner = pool.clone();
    inner.extend([st, sq, si]);
    grow(&mut b, &mut r, &mut inner, 60);
    let nt = *inner.iter().rev().find(|&&x| b.nodes[x as usize].ty == Ty::F32).unwrap_or(&st);
    let nq = *inner.iter().rev().find(|&&x| b.nodes[x as usize].ty == Ty::V3).unwrap_or(&sq);
    let nt = b.node(Op::Prim(Prim::Add), Ty::F32, &[nt, si]);
    let bound = b.lit(&[40.0], Ty::F32);
    let stop = b.node(Op::Prim(Prim::Gt), Ty::Bool, &[st, bound]);
    let count = b.lit(&[24.0], Ty::F32);
    let lp = b.decl(Tag::Loop, 2, &[count, t0, p, st, sq, si, nt, nq, stop], [0, 0]);
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
            Ty::V2 | Ty::V3 | Ty::V4 | Ty::Rec(_) => b.node(Op::Prim(Prim::Length), Ty::F32, &[x]),
        };
        acc = b.node(Op::Prim(Prim::Add), Ty::F32, &[acc, s]);
    }
    let a = *pool.iter().rev().find(|&&x| b.nodes[x as usize].ty == Ty::V3).unwrap_or(&p);
    let out = b.node(Op::Prim(Prim::Vec), Ty::V4, &[a, acc]);
    let main = b.decl(Tag::Fn, 1, &[p, out], [0, 0]);
    (b, main)
}

fn emit_text(b: &B, main: u32) -> String {
    let g = b.graph();
    match module(&g, &[("paint", main)], &b.at) {
        Ok(m) => {
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

/// A random topological order of `b` (a Call or Loop after its decl's roots), with `dead` discarded nodes mixed in
/// and the decls shuffled: the same graph as another job would record it.
fn permute(b: &B, main: u32, seed: u64, dead: usize) -> (B, u32) {
    let mut r = Rng(seed | 1);
    let mut src = b.clone();
    let mut pool: Vec<u32> = (0..src.nodes.len() as u32)
        .filter(|&n| matches!(src.nodes[n as usize].op, Op::Lit(_) | Op::Leaf(Leaf::Time)))
        .collect();
    grow(&mut src, &mut r, &mut pool, dead);
    let n = src.nodes.len();
    let deps = |x: usize| -> Vec<u32> {
        let node = src.nodes[x];
        let mut d = src.args[node.args as usize..(node.args + node.n) as usize].to_vec();
        if let Op::Loop { decl, .. } | Op::Call(decl) = node.op {
            let dl = src.decls[decl as usize];
            d.extend_from_slice(&src.roots[dl.roots as usize..(dl.roots + dl.n) as usize]);
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
    let mut dorder: Vec<usize> = (0..src.decls.len()).collect();
    for i in (1..dorder.len()).rev() {
        dorder.swap(i, r.below(i + 1));
    }
    let mut dnew = vec![0u32; dorder.len()];
    dorder.iter().enumerate().for_each(|(i, &o)| dnew[o] = i as u32);
    let mut out = B { lits: src.lits.clone(), syms: src.syms.clone(), ends: src.ends.clone(), ..B::default() };
    for &o in &order {
        let node = src.nodes[o];
        let args: Vec<u32> =
            src.args[node.args as usize..(node.args + node.n) as usize].iter().map(|&a| new_of[a as usize]).collect();
        let d = |x: u32| dnew[x as usize];
        let op = match node.op {
            Op::Call(x) => Op::Call(d(x)),
            Op::Loop { decl, k } => Op::Loop { decl: d(decl), k },
            Op::Fetch { kernel, lane } => Op::Fetch { kernel: d(kernel), lane },
            Op::Leaf(Leaf::Param(x)) => Op::Leaf(Leaf::Param(d(x))),
            Op::Leaf(Leaf::Input(x)) => Op::Leaf(Leaf::Input(d(x))),
            other @ (Op::Lit(_) | Op::Leaf(_) | Op::Prim(_) | Op::Swizzle(_) | Op::Grad | Op::Raw(_)) => other,
        };
        out.node(op, node.ty, &args);
    }
    for &o in &dorder {
        let dl = src.decls[o];
        let roots: Vec<u32> =
            src.roots[dl.roots as usize..(dl.roots + dl.n) as usize].iter().map(|&x| new_of[x as usize]).collect();
        out.decl(dl.tag, dl.arity, &roots, src.at[o]);
    }
    (out, dnew[main as usize])
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
    let x = b.node(Op::Leaf(Leaf::Bind(0)), Ty::F32, &[]);
    let s = b.sym("fn wobble(x: f32) -> f32 {\n  let y = x * 2.0\n  return y;\n}");
    let call = b.node(Op::Raw(s), Ty::F32, &[x]);
    b.sites.push(Site { file: "examples/reef.ts".into(), line: 40, col: 3 });
    b.node_sites[call as usize] = 1;
    let f = b.decl(Tag::Fn, 1, &[x, call], [0, 0]);
    let g = b.graph();
    match module(&g, &[("paint", f)], &b.at) {
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
    let x = b.node(Op::Leaf(Leaf::Bind(0)), Ty::F32, &[]);
    let z = b.lit(&[0.0], Ty::F32);
    let c = b.node(Op::Prim(Prim::Gt), Ty::Bool, &[x, z]);
    let e = b.node(Op::Prim(Prim::Exp), Ty::F32, &[x]);
    let s = b.node(Op::Prim(Prim::Sin), Ty::F32, &[e]);
    let sel = b.node(Op::Prim(Prim::Select), Ty::F32, &[c, s, z]);
    let f = b.decl(Tag::Fn, 1, &[x, sel], [0, 0]);
    let t = emit_text(&b, f);
    println!("{t}");
    assert!(t.contains("if (v0) {\n    let v1 = exp(a0);\n    let v2 = sin(v1);"), "exp and sin move under the if");
}

/// Two raw sources that declare the same function name are refused (Naga: redefinition), never silently merged so
/// that the second author's calls run the first author's body.
#[test]
fn two_raw_sources_with_one_name_are_refused() {
    let mut b = B::default();
    let x = b.node(Op::Leaf(Leaf::Bind(0)), Ty::F32, &[]);
    let s1 = b.sym("fn wobble(x: f32) -> f32 {\n  return x * 2.0;\n}");
    let s2 = b.sym("fn wobble(x: f32) -> f32 {\n  return x * 3.0;\n}");
    let c1 = b.node(Op::Raw(s1), Ty::F32, &[x]);
    let c2 = b.node(Op::Raw(s2), Ty::F32, &[x]);
    let sum = b.node(Op::Prim(Prim::Add), Ty::F32, &[c1, c2]);
    let f = b.decl(Tag::Fn, 1, &[x, sum], [0, 0]);
    let g = b.graph();
    match module(&g, &[("paint", f)], &b.at) {
        Err(Error::Scene { fault: SceneFault::Wgsl { msg, .. }, .. }) => println!("refused: {msg}"),
        other => panic!("expected Scene{{Wgsl}}, got {:?}", other.map(|m| m.text)),
    }
}
