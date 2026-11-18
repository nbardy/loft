// The recorder (docs/proposals/2026-11-19_lean_rust_loft.md section 3.2). Every node src/expr.ts makes is appended to
// one log, by `Expr.fresh`, as the wire's four words: from then on the log alone is the program, and `job` writes the
// part of it the roots reach as the wire's spans, in a canonical order (section 3.3): nodes numbered by first use in a
// post-order walk from the roots, decls numbered when first reached, loop and graph-function binders named by their
// owner's number, literals, symbols and resources in first-use order. So creation order (import order, bytecode or
// source, discarded expressions) never reaches the job.
//
// While the TS engine is the default it still interns, so the log holds each distinct node once and node k of the log
// is `Expr` id k. A text graph function called with expressions is recorded once, with binders (lowered when a job is
// written), and each call as one `Call` node (src/text/lower.ts); the call's inlined nodes stay in the log for the TS
// engine, and the result is aliased to the `Call`. `made` counts what a recorder that does not intern appends (R in section 8.1): each
// construction outside a recorded call's inlining, plus one per call.

import type { Expr, Fn, InputSpec, LibFn, Loop, ParamSpec, Row, TableSource, Ty } from "../expr.ts";
import { BOUND, Decl, FREE, Fill, Op, PRIM0, PRIMS, ROLES, swizzleCode, TYS, type Job, type Program } from "./wire.ts";
import type { Camera, Check, Cover, Draw, Formula, Medium, Pass, Role, Scene, Solve } from "./decls.ts";

const PRIM = Object.fromEntries(PRIMS.map((p, k) => [p, PRIM0 + k])) as Record<Fn, number>;
const TY = Object.fromEntries(TYS.map((t, k) => [t, k])) as Record<Ty, number>;
const head = (op: number, ty: Ty, flags = 0): number => op | (TY[ty] << 8) | (flags << 16);

/** What a node's `imm` names in the log, by node kind. */
type Leaf = string | ParamSpec | InputSpec | Row | TableSource | LibFn;
type LoopRec = { readonly count: number; readonly init: readonly number[]; readonly binders: readonly number[]; readonly next: readonly number[]; readonly stop: number | undefined };
/** A graph function's body over fresh binders, or why it has none (its calls are then written inlined). */
type FnBody = { readonly kind: "body"; readonly binders: readonly Expr[]; readonly body: Expr } | { readonly kind: "inline"; readonly why: string };
type Lowered = { readonly kind: "body"; readonly ret: Ty; readonly binders: readonly number[]; readonly body: number } | { readonly kind: "inline"; readonly why: string };
type FnRec = { readonly name: string; readonly lower: () => FnBody; done: Lowered | undefined };
type CallRec = { readonly fn: number; readonly args: readonly number[]; readonly result: number };

class Grow<A extends Uint32Array | Float64Array> {
  n = 0;
  constructor(public a: A) {}
  push(...xs: number[]): number {
    const at = this.n;
    if (at + xs.length > this.a.length) { const b = new (this.a.constructor as new (n: number) => A)(2 * this.a.length + xs.length); b.set(this.a); this.a = b; }
    this.a.set(xs, at); this.n += xs.length;
    return at;
  }
}

export class Recorder {
  /** Constructions a non-interning recorder appends (R). */
  made = 0;
  /** Constructions inside a compiler rewrite (`replace`): work that moves into loft-core, so not part of R. */
  rewritten = 0;
  private mutes = 0;
  private rewrites = 0;
  private readonly nodes = new Grow(new Uint32Array(1 << 14));
  private readonly args = new Grow(new Uint32Array(1 << 14));
  private readonly lits = new Grow(new Float64Array(1 << 12));
  private readonly leaves: Leaf[] = [];
  private readonly loops: LoopRec[] = [];
  private readonly loopOf = new Map<Loop, number>();
  private readonly fns: FnRec[] = [];
  private readonly calls: CallRec[] = [];
  /** A call's inlined result (an `Expr` id) to its `Call` (encoded -1 - k). */
  private readonly alias = new Map<number, number>();
  private capture: { roots: Expr[]; seen: Set<number> } | undefined;
  /** src/expr.ts `variable`, which src/expr.ts hands over when it loads: this module cannot import a value from
   * expr.ts, which imports `rec` from here and uses it while it loads. A fill's binders are made with it. */
  private variable: Variable | undefined;
  useVariables(v: Variable): void { this.variable = v; }

  /** One construction (an interning lookup, hit or miss). */
  construct(): void { if (this.rewrites > 0) this.rewritten++; else if (this.mutes === 0) this.made++; }
  /** The number of nodes in the log: the id the next node gets. */
  get size(): number { return this.nodes.n >> 2; }

  /** Append `e`, just made (`Expr.fresh`): its id is the log's next index. */
  node(e: Expr): void {
    const n = e.node, ids = (xs: readonly Expr[]): number => this.args.push(...xs.map((x) => x.id));
    const leaf = (x: Leaf): number => this.leaves.push(x) - 1;
    switch (n.op) {
      case "const": this.nodes.push(head(Op.Lit, n.ty), 0, n.v.length, this.lits.push(...n.v)); break;
      case "var": this.nodes.push(head(Op.Bind, n.ty, FREE), 0, 0, leaf(n.name)); break;
      case "input": this.nodes.push(n.spec.name === "time" ? head(Op.Time, n.ty) : head(Op.Input, n.ty), 0, 0, leaf(n.spec)); break;
      case "param": this.nodes.push(head(Op.Param, n.ty), 0, 0, leaf(n.spec)); break;
      case "row": this.nodes.push(head(Op.Fetch, n.ty), ids([n.x]), 1, leaf(n.row)); break;
      case "fetch": this.nodes.push(head(Op.Fetch, n.ty, n.lane), ids([n.index]), 1, leaf(n.table)); break;
      case "call": this.nodes.push(head(PRIM[n.fn], n.ty), ids(n.args), n.args.length, 0); break;
      case "vec": this.nodes.push(head(Op.Vec, n.ty), ids(n.args), n.args.length, 0); break;
      case "get": this.nodes.push(head(Op.Swizzle, n.ty), ids([n.a]), 1, swizzleCode(n.sel)); break;
      case "lib": this.nodes.push(head(Op.Raw, n.ty), ids(n.args), n.args.length, leaf(n.fn)); break;
      case "iterate": this.nodes.push(head(Op.Loop, n.ty, n.k), 0, 0, this.loopRec(n.loop)); break;
    }
  }

  private loopRec(l: Loop): number {
    let k = this.loopOf.get(l);
    if (k === undefined) {
      this.loopOf.set(l, (k = this.loops.length));
      this.loops.push({ count: l.count.id, init: l.init.map((x) => x.id), binders: [...l.state, l.index].map((x) => x.id), next: l.next.map((x) => x.id), stop: l.stop?.id });
    }
    return k;
  }

  // ---- graph functions (src/text/lower.ts) ----

  /**
   * A graph function: its number for `call`. Its body is lowered over binders only when a job is written, after the TS
   * engine has emitted: nodes made earlier would shift every later id, and the TS engine's WGSL names nodes by id.
   */
  fn(name: string, lower: () => FnBody): number { return this.fns.push({ name, lower, done: undefined }) - 1; }
  /** One call of graph function `fn`; `result` (its inlined value) is read as the call when it was made after `since`. */
  call(fn: number, args: readonly Expr[], result: Expr, since: number): void {
    // Counted like a construction: a call nested in a recorded call's inlining is TS-engine work, not part of R.
    this.construct();
    const k = this.calls.push({ fn, args: args.map((a) => a.id), result: result.id }) - 1;
    if (result.id >= since) this.alias.set(result.id, -1 - k);
  }
  /** `f` with constructions uncounted (a recorded call's inlining, which only the TS engine reads). */
  muted<T>(f: () => T): T { this.mutes++; try { return f(); } finally { this.mutes--; } }
  /** `f` counted as a rewrite, not as recording. */
  rewrite<T>(f: () => T): T { this.rewrites++; try { return f(); } finally { this.rewrites--; } }
  /** `f` counted (a graph function's one recording, even when first reached inside a muted call). */
  counted<T>(f: () => T): T { const m = this.mutes; this.mutes = 0; try { return f(); } finally { this.mutes = m; } }

  // ---- the TS engine's roots, for the shadow comparison ----

  /** Collect every root `emitWgsl` is given until the returned function is called; it gives them, first seen first. */
  watch(): () => Expr[] {
    const c = (this.capture = { roots: [], seen: new Set() });
    return () => { this.capture = undefined; return c.roots; };
  }
  emitted(roots: readonly Expr[]): void {
    const c = this.capture;
    if (c !== undefined) for (const r of roots) if (!c.seen.has(r.id)) { c.seen.add(r.id); c.roots.push(r); }
  }

  /** The job for `program` over these roots, with the scene-level decls `scene` (none by default). */
  job(program: Program, roots: readonly Expr[], scene: Scene = { passes: [], checks: [], solves: [] }): Job {
    this.settle();
    const variable = this.variable;
    if (variable === undefined) throw new Error("internal: src/expr.ts has not handed the recorder `variable`");
    return new Writer(this.view(), { settle: () => this.settle(), quiet: (f) => this.muted(f), variable }).write(program, roots.map((r) => r.id), scene);
  }

  /** Lower every graph function not yet lowered. Lowering a body may call (and so record) further functions: the loop runs until none is pending. */
  private settle(): void {
    for (let k = 0; k < this.fns.length; k++) {
      const f = this.fns[k]!;
      if (f.done !== undefined) continue;
      const b = this.counted(f.lower);
      switch (b.kind) {
        case "body": f.done = { kind: "body", ret: b.body.ty, binders: b.binders.map((x) => x.id), body: b.body.id }; break;
        case "inline": f.done = b; break;
      }
    }
  }

  // The log's typed arrays are read through getters: a table's fill is built while the job is written, and its new
  // nodes may grow (so replace) an array the writer would otherwise still hold.
  private view(): LogView {
    const r = this;
    return {
      get nodes() { return r.nodes.a; }, get args() { return r.args.a; }, get lits() { return r.lits.a; },
      leaves: this.leaves, loops: this.loops, fns: this.fns, calls: this.calls, alias: this.alias,
    };
  }
}

/** What the writer asks of its recorder while it builds fill bodies: lower functions those bodies called, and build uncounted. */
type Variable = <T extends Ty>(name: string, ty: T) => Expr<T>;
type Host = { readonly settle: () => void; readonly quiet: <T>(f: () => T) => T; readonly variable: Variable };

type LogView = {
  readonly nodes: Uint32Array; readonly args: Uint32Array; readonly lits: Float64Array; readonly leaves: readonly Leaf[];
  readonly loops: readonly LoopRec[]; readonly fns: readonly FnRec[]; readonly calls: readonly CallRec[]; readonly alias: ReadonlyMap<number, number>;
};

/** One decl being written: reserved when first reached (its number names its binders), filled when complete. */
type DeclOut = { tag: Decl; words: number[]; roots: number[] };

class Writer {
  private readonly nodes: number[] = [];
  private readonly args: number[] = [];
  private readonly lits: number[] = [];
  private readonly rows: number[] = [];
  private readonly syms = new Map<string, number>();
  private readonly decls: DeclOut[] = [];
  private readonly count = new Map<Decl, number>();
  private readonly out = new Map<number, number>();
  private readonly declOf = new Map<unknown, number>();
  constructor(private readonly log: LogView, private readonly host: Host) {}

  write(program: Program, roots: readonly number[], scene: Scene): Job {
    const top = roots.map((r) => this.visit(r));
    for (const p of scene.passes) this.pass(p);
    for (const c of scene.checks) this.check(c);
    for (const s of scene.solves) this.solve(s);
    const symList = [...this.syms.keys()], bytes = symList.map((s) => new TextEncoder().encode(s));
    const symAt = new Uint32Array(symList.length + 1);
    bytes.forEach((b, k) => { symAt[k + 1] = symAt[k]! + b.length; });
    const syms = new Uint8Array(symAt[symList.length]!);
    bytes.forEach((b, k) => syms.set(b, symAt[k]!));
    const decls = [top.length, ...this.decls.flatMap((d) => [d.tag, d.roots.length, d.words.length, ...d.words])];
    return {
      program, nodes: Uint32Array.from(this.nodes), args: Uint32Array.from(this.args), lits: Float64Array.from(this.lits), rows: Float32Array.from(this.rows),
      blobs: new Uint8Array(0), syms, symAt, sites: new Uint32Array(0), siteOf: new Uint32Array(this.nodes.length >> 2),
      decls: Uint32Array.from(decls), roots: Uint32Array.from([...top, ...this.decls.flatMap((d) => d.roots)]),
    };
  }

  private sym(s: string): number {
    let k = this.syms.get(s);
    if (k === undefined) this.syms.set(s, (k = this.syms.size));
    return k;
  }
  private emit(h: number, kids: readonly number[], imm: number, n = kids.length): number {
    const at = kids.length === 0 ? 0 : this.args.length;
    this.args.push(...kids);
    return (this.nodes.push(h, at, n, imm) >> 2) - 1;
  }
  /** Reserve the next decl of `tag` for `key`: its ordinal among decls of that tag. */
  private reserve(tag: Decl, key: unknown): { k: number; d: DeclOut } {
    const k = this.count.get(tag) ?? 0;
    this.count.set(tag, k + 1);
    const d: DeclOut = { tag, words: [], roots: [] };
    this.decls.push(d);
    this.declOf.set(key, k);
    return { k, d };
  }
  private binder(id: number, name: string): void {
    const h = this.log.nodes[4 * id]!;
    this.out.set(id, this.emit((h & 0xff00) | Op.Bind | (BOUND << 16), [], this.sym(name)));
  }

  private visit(id0: number): number {
    const id = this.log.alias.get(id0) ?? id0;
    const seen = this.out.get(id);
    if (seen !== undefined) return seen;
    const made = id < 0 ? this.callNode(-1 - id) : this.logNode(id);
    this.out.set(id, made);
    return made;
  }

  private callNode(k: number): number {
    // A call made while a fill body was built (after the job began) may name a function not yet lowered.
    if (this.log.fns[this.log.calls[k]!.fn]!.done === undefined) this.host.settle();
    const c = this.log.calls[k]!, f = this.log.fns[c.fn]!.done!;
    switch (f.kind) {
      // A function with no body over binders: this call is its inlined nodes, which the log kept.
      case "inline": return this.logNode(c.result);
      case "body": return this.emit(Op.Call | (TY[f.ret] << 8), c.args.map((a) => this.visit(a)), this.graphFn(c.fn, f));
    }
  }

  private logNode(id: number): number {
    const L = this.log, h = L.nodes[4 * id]!, at = L.nodes[4 * id + 1]!, n = L.nodes[4 * id + 2]!, imm = L.nodes[4 * id + 3]!;
    const op = h & 0xff, kids = (): number[] => Array.from(L.args.subarray(at, at + n), (a) => this.visit(a));
    if (op >= PRIM0) return this.emit(h, kids(), imm);
    switch (op as Op) {
      case Op.Lit: return this.emit(h, [], this.lits.push(...L.lits.subarray(imm, imm + n)) - n, n);
      case Op.Time: return this.emit(h, [], 0);
      case Op.Param: return this.emit(h, [], this.param(L.leaves[imm] as ParamSpec));
      case Op.Input: return this.emit(h, [], this.input(L.leaves[imm] as InputSpec, (h >> 8) & 0xff));
      case Op.Bind: return this.emit(h, [], this.sym(L.leaves[imm] as string));
      case Op.Fetch: { const ks = kids(); return this.emit(h, ks, this.kernel(L.leaves[imm] as Row | TableSource)); }
      case Op.Raw: { const ks = kids(); return this.emit(h, ks, this.rawFn(L.leaves[imm] as LibFn)); }
      case Op.Loop: return this.emit(h, [], this.loop(imm));
      case Op.Swizzle: case Op.Vec: return this.emit(h, kids(), imm);
      case Op.Call: case Op.Grad: throw new Error(`internal: the log holds no ${Op[op]} node (calls live beside it)`);
    }
  }

  private param(p: ParamSpec): number {
    const seen = this.declOf.get(p);
    if (seen !== undefined) return seen;
    const { k, d } = this.reserve(Decl.Param, p);
    d.words.push(this.sym(p.name), this.lits.push(p.def) - 1, this.lits.push(p.min) - 1, this.lits.push(p.max) - 1);
    return k;
  }
  private input(i: InputSpec, ty: number): number {
    const seen = this.declOf.get(i);
    if (seen !== undefined) return seen;
    const { k, d } = this.reserve(Decl.Input, i);
    d.words.push(this.sym(i.name), ty, this.lits.push(...i.def) - i.def.length);
    return k;
  }
  private kernel(r: Row | TableSource): number {
    const seen = this.declOf.get(r);
    if (seen !== undefined) return seen;
    const { k, d } = this.reserve(Decl.Kernel, r);
    if ("samples" in r) { d.words.push(Fill.Rows, this.sym(r.label), this.rows.length, r.samples.length); this.pushRows(r.samples); return k; }
    const head = [this.sym(r.name), r.lanes, r.entries, ROLES.indexOf(r.role)];
    if ("data" in r) { d.words.push(Fill.Data, ...head, this.rows.length); this.pushRows(r.data); return k; }
    if ("recurrence" in r) { d.words.push(Fill.Recurrence, ...head, this.sym(r.recurrence)); return k; }
    // A program's bodies are built here, over fresh variables written as this kernel's binders (K3r0i, K3s0, K3i).
    const p = r.program, fresh = <T extends Ty>(name: string, ty: T): Expr<T> => { const v = this.host.variable(`${name}@${r.name}`, ty); this.binder(v.id, name); return v; };
    switch (p.kind) {
      case "map": {
        d.words.push(Fill.Map, ...head, p.runs.length);
        p.runs.forEach((run, j) => {
          const i = fresh(`K${k}r${j}i`, "f32"), lanes = this.host.quiet(() => run.lanes(i));
          d.words.push(run.count, this.out.get(i.id)!);
          d.roots.push(...lanes.map((x) => this.visit(x.id)));
        });
        return k;
      }
      case "scan": {
        const init = p.init.map((x) => this.visit(x.id));
        const state = p.init.map((x, j) => fresh(`K${k}s${j}`, x.ty)), i = fresh(`K${k}i`, "f32");
        const step = this.host.quiet(() => p.step(state, i));
        d.words.push(Fill.Scan, ...head, p.first, p.steps, state.length, ...state.map((x) => this.out.get(x.id)!), this.out.get(i.id)!);
        d.roots.push(...init, ...step.next.map((x) => this.visit(x.id)), ...step.lanes.map((x) => this.visit(x.id)));
        return k;
      }
    }
  }
  // ---- scene-level decls (src/record/decls.ts; layouts in loft-ir wire.rs) ----

  private lit(...xs: number[]): number { return this.lits.push(...xs) - xs.length; }
  /** A role's Fn ordinal (its graph function, lowered now if it was recorded after the job began). */
  private role(r: Role): number {
    if (this.log.fns[r.fn]!.done === undefined) this.host.settle();
    const f = this.log.fns[r.fn]!.done!;
    switch (f.kind) {
      case "body": return this.graphFn(r.fn, f);
      case "inline": throw new Error(`internal: role ${this.log.fns[r.fn]!.name} has no body over binders (${f.why})`);
    }
  }
  private camera(c: Camera): number {
    const seen = this.declOf.get(c);
    if (seen !== undefined) return seen;
    const roots = c.kind === "look" ? [c.eye, c.target, c.up, c.fovy] : [...c.cols];
    const ids = roots.map((x) => this.visit(x.id));
    const { k, d } = this.reserve(Decl.Camera, c);
    d.words.push(this.sym(c.name), c.kind === "look" ? 0 : 1);
    d.roots.push(...ids);
    return k;
  }
  private medium(m: Medium): number {
    const seen = this.declOf.get(m);
    if (seen !== undefined) return seen;
    const words = m.kind === "march"
      ? [0, this.role(m.density), this.lit(...m.within[0], ...m.within[1]), m.steps, m.shadowSteps]
      : [1, this.kernel(m.table)];
    const { k, d } = this.reserve(Decl.Medium, m);
    d.words.push(...words);
    return k;
  }
  private cover(c: Cover): number[] {
    switch (c.kind) {
      case "grid": return [0, c.u, c.v, this.role(c.place), c.count];
      case "mesh": return [1, this.kernel(c.table), c.count];
      case "quads": return [2, this.role(c.place), c.count];
      case "ray": {
        const find = c.find.kind === "hit" ? [0, this.role(c.find.hit)] : [1, this.medium(c.find.medium)];
        switch (c.proxy.kind) {
          case "box": return [3, ...find, this.lit(...c.proxy.min, ...c.proxy.max)];
          case "instances": return [4, ...find, this.kernel(c.proxy.table), this.lit(...c.proxy.local[0], ...c.proxy.local[1])];
        }
      }
      // falls through: no proxy kind is left
      case "screen": return [5];
    }
  }
  private draw(x: Draw): number {
    const paint = this.role(x.paint), cover = this.cover(x.cover);
    const { k, d } = this.reserve(Decl.Draw, x);
    const blend = ["opaque", "alpha", "add"].indexOf(x.raster.blend);
    d.words.push(this.sym(x.name), x.layer === "subject" ? 0 : 1, Number(x.raster.cull), Number(x.raster.depthWrite), blend, paint, ...cover);
    return k;
  }
  private pass(p: Pass): void {
    const camera = this.camera(p.camera), first = this.count.get(Decl.Draw) ?? 0;
    // A pass's draws are consecutive Draw ordinals: each draw is written here, once, for its pass.
    for (const x of p.draws) this.draw(x);
    const end = this.count.get(Decl.Draw) ?? 0;
    const clear = p.clear.kind === "load" ? [0] : [1, this.lit(...p.clear.rgba)];
    const ground = p.ground.kind === "none" ? [0] : [1, this.role(p.ground.hit), this.role(p.ground.paint)];
    const outline = p.outline.kind === "none" ? [0] : [1, this.role(p.outline.outline)];
    const { d } = this.reserve(Decl.Pass, p);
    d.words.push(camera, first, end, ...clear, ...ground, ...outline);
  }
  /** A formula's words, pre-order, with its roots appended to `roots` in the same order. */
  private formula(f: Formula, words: number[], roots: number[]): void {
    switch (f.kind) {
      case "atom": words.push(0); roots.push(this.visit(f.margin.id), this.visit(f.test.id)); return;
      case "always": case "eventually":
        words.push(f.kind === "always" ? 1 : 2, f.leaves.length, ...f.leaves.map((l) => this.visit(l.leaf.id)));
        roots.push(...f.leaves.map((l) => this.visit(l.lo.id)), ...f.leaves.map((l) => this.visit(l.hi.id)));
        this.formula(f.body, words, roots);
        return;
      case "and": case "or":
        words.push(f.kind === "and" ? 3 : 4, f.parts.length);
        for (const p of f.parts) this.formula(p, words, roots);
        return;
      case "not": words.push(5); this.formula(f.part, words, roots); return;
    }
  }
  private check(c: Check): void {
    const words = [this.sym(c.label), c.how.kind === "prove" ? 0 : 1, c.how.kind === "prove" ? c.how.boxes : c.how.n], roots: number[] = [];
    this.formula(c.body, words, roots);
    const { d } = this.reserve(Decl.Check, c);
    d.words.push(...words);
    d.roots.push(...roots);
  }
  private solve(s: Solve): void {
    const from = s.unknowns.map((u) => this.visit(u.from.id)), to = s.near.map((n) => this.visit(n.to.id));
    const { k, d } = this.reserve(Decl.Solve, s);
    // The jets are this solve's binders (S0u1j2: solve 0, unknown 1, derivative 2), read only by its margins.
    s.unknowns.forEach((u, j) => u.jets.forEach((x, o) => this.binder(x.id, `S${k}u${j}j${o}`)));
    const margins = s.holds.map((h) => this.visit(h.margin.id));
    d.words.push(this.sym(s.label), this.lit(s.over[0], s.over[1], s.delta, s.jerk, s.acceleration, s.nearWeight), s.unknowns.length);
    for (const u of s.unknowns) d.words.push(this.sym(u.name), u.numbers, ...u.jets.map((x) => this.out.get(x.id)!));
    d.words.push(s.near.length, ...s.near.map((n) => n.unknown), s.holds.length);
    for (const h of s.holds) {
      d.words.push(...(h.kind === "equal" ? [0, this.lit(h.within)] : [1]));
      d.words.push(...(h.times.kind === "at" ? [0, this.lit(h.times.t)] : h.times.kind === "during" ? [1, this.lit(h.times.from, h.times.to)] : [2]));
      d.words.push(this.sym(h.said), this.sym(h.site));
    }
    d.roots.push(...from, ...to, ...margins);
  }

  private pushRows(xs: ArrayLike<number>): void { for (let j = 0; j < xs.length; j++) this.rows.push(xs[j]!); }
  private rawFn(f: LibFn): number {
    const seen = this.declOf.get(f);
    if (seen !== undefined) return seen;
    const deps = (f.deps ?? []).map((x) => this.rawFn(x));
    const { k, d } = this.reserve(Decl.Fn, f);
    d.words.push(1, this.sym(f.name), TY[f.ret], this.sym(f.wgsl), deps.length, ...deps);
    return k;
  }
  private graphFn(fn: number, f: Lowered & { kind: "body" }): number {
    const key = this.log.fns[fn]!, seen = this.declOf.get(key);
    if (seen !== undefined) return seen;
    const { k, d } = this.reserve(Decl.Fn, key);
    f.binders.forEach((b, j) => this.binder(b, `F${k}a${j}`));
    d.roots.push(this.visit(f.body));
    d.words.push(0, this.sym(key.name), TY[f.ret], f.binders.length, ...f.binders.map((b) => this.out.get(b)!));
    return k;
  }
  private loop(rec: number): number {
    const l = this.log.loops[rec]!, seen = this.declOf.get(l);
    if (seen !== undefined) return seen;
    const count = this.visit(l.count), init = l.init.map((x) => this.visit(x));
    const { k, d } = this.reserve(Decl.Loop, l);
    const last = l.binders.length - 1;
    l.binders.forEach((b, j) => this.binder(b, j === last ? `L${k}i` : `L${k}s${j}`));
    const next = l.next.map((x) => this.visit(x)), stop = l.stop === undefined ? [] : [this.visit(l.stop)];
    d.roots.push(count, ...init, ...next, ...stop);
    d.words.push(l.init.length, stop.length, ...l.binders.map((b) => this.out.get(b)!));
    return k;
  }
}

/** The process's recorder: src/expr.ts appends to it. */
export const rec = new Recorder();
