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

import type { Expr, Fn, InputSpec, LibFn, LibRead, Loop, ParamSpec, Row, TableSource, Ty } from "../expr.ts";
import type { ComputePass } from "../compute.ts";
import type { PassDef } from "../frame/pass.ts";
import type { RecurPart, RecurState } from "../map/recur.ts";
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

/** A typed array appended to in place: no rest arguments, so a push allocates nothing (the recorder's hot path). */
class Grow<A extends Uint32Array | Int32Array | Float64Array | Float32Array> {
  n = 0;
  constructor(public a: A) {}
  private room(k: number): void {
    if (this.n + k <= this.a.length) return;
    const b = new (this.a.constructor as new (n: number) => A)(2 * this.a.length + k);
    b.set(this.a);
    this.a = b;
  }
  push1(x: number): number { this.room(1); this.a[this.n] = x; return this.n++; }
  push4(x: number, y: number, z: number, w: number): number {
    this.room(4);
    const a = this.a, at = this.n;
    a[at] = x; a[at + 1] = y; a[at + 2] = z; a[at + 3] = w;
    this.n = at + 4;
    return at;
  }
  /** xs[from .. to) appended; the offset of the first. */
  pushAll(xs: ArrayLike<number>, from = 0, to = xs.length): number {
    const at = this.n;
    this.room(to - from);
    for (let j = from; j < to; j++) this.a[this.n++] = xs[j]!;
    return at;
  }
  /** The first `n` elements, copied. */
  done(): A { return this.a.slice(0, this.n) as A; }
}

/** An int per id, -1 until set, growing on demand (log ids keep growing while a job is written). */
class Ints {
  private a = new Int32Array(1 << 12).fill(-1);
  get(i: number): number { return i < this.a.length ? this.a[i]! : -1; }
  set(i: number, v: number): void {
    if (i >= this.a.length) { const b = new Int32Array(Math.max(2 * this.a.length, i + 1)).fill(-1); b.set(this.a); this.a = b; }
    this.a[i] = v;
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
  /** A call's inlined result (an `Expr` id) to its `Call` k, as k (-1: none). */
  private readonly alias = new Ints();
  private capture: { roots: Expr[]; seen: Set<number> } | undefined;
  /** src/expr.ts `variable`, which src/expr.ts hands over when it loads: this module cannot import a value from
   * expr.ts, which imports `rec` from here and uses it while it loads. A fill's binders are made with it. */
  private variable: Variable | undefined;
  useVariables(v: Variable): void { this.variable = v; }
  /** src/map/recur.ts `recurTable`, handed over when that module loads (it imports this one). Until it loads no table
   * belongs to a recurrence, so none is asked about. */
  private recurrences: Recurrences = () => undefined;
  useRecurrences(r: Recurrences): void { this.recurrences = r; }

  /** One construction (an interning lookup, hit or miss). */
  construct(): void { if (this.rewrites > 0) this.rewritten++; else if (this.mutes === 0) this.made++; }
  /** The number of nodes in the log: the id the next node gets. */
  get size(): number { return this.nodes.n >> 2; }

  /** Append `e`, just made (`Expr.fresh`): its id is the log's next index. */
  node(e: Expr): void {
    const n = e.node, A = this.args, N = this.nodes;
    const ids = (xs: readonly Expr[]): number => { const at = A.n; for (let j = 0; j < xs.length; j++) A.push1(xs[j]!.id); return at; };
    const leaf = (x: Leaf): number => this.leaves.push(x) - 1;
    switch (n.op) {
      case "const": N.push4(head(Op.Lit, n.ty), 0, n.v.length, this.lits.pushAll(n.v)); break;
      case "var": N.push4(head(Op.Bind, n.ty, FREE), 0, 0, leaf(n.name)); break;
      case "input": N.push4(n.spec.name === "time" ? head(Op.Time, n.ty) : head(Op.Input, n.ty), 0, 0, leaf(n.spec)); break;
      case "param": N.push4(head(Op.Param, n.ty), 0, 0, leaf(n.spec)); break;
      case "row": N.push4(head(Op.Fetch, n.ty), A.push1(n.x.id), 1, leaf(n.row)); break;
      case "fetch": N.push4(head(Op.Fetch, n.ty, n.lane), A.push1(n.index.id), 1, leaf(n.table)); break;
      case "call": N.push4(head(PRIM[n.fn], n.ty), ids(n.args), n.args.length, 0); break;
      case "vec": N.push4(head(Op.Vec, n.ty), ids(n.args), n.args.length, 0); break;
      case "get": N.push4(head(Op.Swizzle, n.ty), A.push1(n.a.id), 1, swizzleCode(n.sel)); break;
      case "lib": N.push4(head(Op.Raw, n.ty), ids(n.args), n.args.length, leaf(n.fn)); break;
      case "iterate": N.push4(head(Op.Loop, n.ty, n.k), 0, 0, this.loopRec(n.loop)); break;
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
    if (result.id >= since) this.alias.set(result.id, k);
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
  /** `f` with `watch` suspended: what it emits is a decl's body the job carries itself, not a program root. */
  unwatched<T>(f: () => T): T {
    const c = this.capture;
    this.capture = undefined;
    try { return f(); } finally { this.capture = c; }
  }
  emitted(roots: readonly Expr[]): void {
    const c = this.capture;
    if (c !== undefined) for (const r of roots) if (!c.seen.has(r.id)) { c.seen.add(r.id); c.roots.push(r); }
  }

  /** The job for `program` over these roots, with the scene-level decls `scene` (none by default). */
  job(program: Program, roots: readonly Expr[], scene: Scene = { cameras: [], passes: [], checks: [], solves: [] }): Job {
    this.settle();
    const variable = this.variable;
    if (variable === undefined) throw new Error("internal: src/expr.ts has not handed the recorder `variable`");
    return new Writer(this.view(), { settle: () => this.settle(), quiet: (f) => this.muted(f), variable, recurrences: this.recurrences }).write(program, roots.map((r) => r.id), scene);
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
type Recurrences = (t: TableSource) => { readonly state: RecurState; readonly part: RecurPart } | undefined;
type Host = { readonly settle: () => void; readonly quiet: <T>(f: () => T) => T; readonly variable: Variable; readonly recurrences: Recurrences };

/** A thing the TS engine can build that the wire cannot carry yet: the job is not written, by name. */
export class Unrecorded extends Error {
  override readonly name = "Unrecorded";
}
const PARTS: readonly RecurPart[] = ["kept", "prev", "even", "odd"];
const NONE: readonly number[] = [];
/** The UTF-8 length of `s` (lone surrogates encode as U+FFFD, three bytes, as TextEncoder writes them). */
function utf8Length(s: string): number {
  let n = 0;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c < 0x80) n += 1;
    else if (c < 0x800) n += 2;
    else if (c >= 0xd800 && c < 0xdc00 && i + 1 < s.length && (s.charCodeAt(i + 1) & 0xfc00) === 0xdc00) { n += 4; i++; }
    else n += 3;
  }
  return n;
}
const WHEN = { build: 0, load: 1, frame: 2, steps: 3 } as const;
const KEEP = { colour: 0, depth: 1, light: 2 } as const;

type LogView = {
  readonly nodes: Uint32Array; readonly args: Uint32Array; readonly lits: Float64Array; readonly leaves: readonly Leaf[];
  readonly loops: readonly LoopRec[]; readonly fns: readonly FnRec[]; readonly calls: readonly CallRec[]; readonly alias: Ints;
};

/** One decl being written: reserved when first reached (its number names its binders), filled when complete. */
type DeclOut = { tag: Decl; words: number[]; roots: number[] };

class Writer {
  private readonly nodes = new Grow(new Uint32Array(1 << 14));
  private readonly args = new Grow(new Uint32Array(1 << 14));
  private readonly lits = new Grow(new Float64Array(1 << 10));
  private readonly rows = new Grow(new Float32Array(1 << 10));
  private readonly syms = new Map<string, number>();
  private readonly decls: DeclOut[] = [];
  private readonly count = new Map<Decl, number>();
  /** Log node id to job node id, and call k to its job node: -1 until written. */
  private readonly out = new Ints();
  private readonly outCall = new Ints();
  private readonly declOf = new Map<unknown, number>();
  constructor(private readonly log: LogView, private readonly host: Host) {}

  write(program: Program, roots: readonly number[], scene: Scene): Job {
    const top = roots.map((r) => this.visit(r));
    for (const s of scene.rowSteps ?? []) {
      const input = this.input(s.input, TY[s.input.ty]);
      const home = () => { switch (s.home.kind) {
        case "level": return [0, 0];
        case "script": return [1, this.kernel(s.home.samples)];
        case "events": return [2, s.home.slots];
      } };
      const words = [this.sym(s.source), this.kernel(s.plan), this.kernel(s.seed), this.kernel(s.output), input, s.hz, s.ticks, ...home()];
      this.reserve(Decl.RowStep, s).d.words.push(...words);
    }
    for (const c of scene.cameras) this.camera(c);
    for (const p of scene.passes) this.pass(p);
    for (const c of scene.checks) this.check(c);
    for (const s of scene.solves) this.solve(s);
    // Symbols: one UTF-8 encoding of them all, its offsets per symbol (each symbol's length in UTF-8 bytes).
    const symList = [...this.syms.keys()], all = new TextEncoder().encode(symList.join(""));
    const symAt = new Uint32Array(symList.length + 1);
    for (let k = 0; k < symList.length; k++) symAt[k + 1] = symAt[k]! + utf8Length(symList[k]!);
    const decls = new Grow(new Uint32Array(1 + this.decls.length * 4)), declRoots = new Grow(new Uint32Array(top.length + this.decls.length));
    decls.push1(top.length);
    declRoots.pushAll(top);
    for (const d of this.decls) { decls.push1(d.tag); decls.push1(d.roots.length); decls.push1(d.words.length); decls.pushAll(d.words); declRoots.pushAll(d.roots); }
    return {
      ...(scene.rowSteps?.length ? { wire: 5 as const } : {}),
      program, nodes: this.nodes.done(), args: this.args.done(), lits: this.lits.done(), rows: this.rows.done(),
      blobs: new Uint8Array(0), syms: all, symAt, sites: new Uint32Array(0), siteOf: new Uint32Array(this.nodes.n >> 2),
      decls: decls.done(), roots: declRoots.done(),
    };
  }

  private sym(s: string): number {
    let k = this.syms.get(s);
    if (k === undefined) this.syms.set(s, (k = this.syms.size));
    return k;
  }
  private emit(h: number, kids: readonly number[], imm: number, n = kids.length): number {
    const at = kids.length === 0 ? 0 : this.args.pushAll(kids);
    return this.nodes.push4(h, at, n, imm) >> 2;
  }
  /** A log node's job node, after it was written (a binder, a run's index). */
  private outOf(id: number): number {
    const k = this.out.get(id);
    if (k < 0) throw new Error(`internal: log node ${id} read before it was written`);
    return k;
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
    this.out.set(id, this.emit((h & 0xff00) | Op.Bind | (BOUND << 16), NONE, this.sym(name)));
  }

  private visit(id: number): number {
    const call = this.log.alias.get(id);
    if (call >= 0) {
      const seen = this.outCall.get(call);
      if (seen >= 0) return seen;
      const made = this.callNode(call);
      this.outCall.set(call, made);
      return made;
    }
    const seen = this.out.get(id);
    if (seen >= 0) return seen;
    const made = this.logNode(id);
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
    const op = h & 0xff, kids = (): number[] => { const ks = new Array<number>(n); for (let j = 0; j < n; j++) ks[j] = this.visit(L.args[at + j]!); return ks; };
    if (op >= PRIM0) return this.emit(h, kids(), imm);
    switch (op as Op) {
      case Op.Lit: return this.emit(h, NONE, this.lits.pushAll(L.lits, imm, imm + n), n);
      case Op.Time: return this.emit(h, NONE, 0);
      case Op.Param: return this.emit(h, NONE, this.param(L.leaves[imm] as ParamSpec));
      case Op.Input: return this.emit(h, NONE, this.input(L.leaves[imm] as InputSpec, (h >> 8) & 0xff));
      case Op.Bind: return this.emit(h, NONE, this.sym(L.leaves[imm] as string));
      case Op.Fetch: { const ks = kids(); return this.emit(h, ks, this.kernel(L.leaves[imm] as Row | TableSource)); }
      case Op.Raw: { const ks = kids(); return this.emit(h, ks, this.rawFn(L.leaves[imm] as LibFn)); }
      case Op.Loop: return this.emit(h, NONE, this.loop(imm));
      case Op.Swizzle: case Op.Vec: return this.emit(h, kids(), imm);
      case Op.Make: case Op.Field: case Op.Open: throw new Unrecorded("typed record operations use the wire 5 kind and open declarations");
      case Op.Call: case Op.Grad: throw new Error(`internal: the log holds no ${Op[op]} node (calls live beside it)`);
    }
  }

  private param(p: ParamSpec): number {
    const seen = this.declOf.get(p);
    if (seen !== undefined) return seen;
    const { k, d } = this.reserve(Decl.Param, p);
    d.words.push(this.sym(p.name), this.lits.push1(p.def), this.lits.push1(p.min), this.lits.push1(p.max));
    return k;
  }
  private input(i: InputSpec, ty: number): number {
    const seen = this.declOf.get(i);
    if (seen !== undefined) return seen;
    const { k, d } = this.reserve(Decl.Input, i);
    d.words.push(this.sym(i.name), ty, this.lits.pushAll(i.def));
    return k;
  }
  private kernel(r: Row | TableSource): number {
    const seen = this.declOf.get(r);
    if (seen !== undefined) return seen;
    const { k, d } = this.reserve(Decl.Kernel, r);
    if ("samples" in r) { d.words.push(Fill.Rows, this.sym(r.label), this.rows.pushAll(r.samples), r.samples.length); return k; }
    const head = [this.sym(r.name), r.lanes, r.entries, ROLES.indexOf(r.role)];
    if ("data" in r) { d.words.push(Fill.Data, ...head, this.rows.pushAll(r.data)); return k; }
    if ("recurrence" in r) { this.recurrence(r, head, k, d); return k; }
    // A program's bodies are built here, over fresh variables written as this kernel's binders (K3r0i, K3s0, K3i).
    const p = r.program, fresh = <T extends Ty>(name: string, ty: T): Expr<T> => { const v = this.host.variable(`${name}@${r.name}`, ty); this.binder(v.id, name); return v; };
    switch (p.kind) {
      case "map": {
        d.words.push(Fill.Map, ...head, p.runs.length);
        p.runs.forEach((run, j) => {
          const i = fresh(`K${k}r${j}i`, "f32"), lanes = this.host.quiet(() => run.lanes(i));
          d.words.push(run.count, this.outOf(i.id));
          d.roots.push(...lanes.map((x) => this.visit(x.id)));
        });
        return k;
      }
      case "scan": {
        const init = p.init.map((x) => this.visit(x.id));
        const state = p.init.map((x, j) => fresh(`K${k}s${j}`, x.ty)), i = fresh(`K${k}i`, "f32");
        const step = this.host.quiet(() => p.step(state, i));
        d.words.push(Fill.Scan, ...head, p.first, p.steps, state.length, ...state.map((x) => this.outOf(x.id)), this.outOf(i.id));
        d.roots.push(...init, ...step.next.map((x) => this.visit(x.id)), ...step.lanes.map((x) => this.visit(x.id)));
        return k;
      }
    }
  }
  /** A recurrence's table (src/map/recur.ts): the kept states carry the recurrence over this kernel's binders (K4i, K4t);
   * a part (its previous state, a replay buffer) names the kept states' kernel. */
  private recurrence(r: TableSource, head: readonly number[], k: number, d: DeclOut): void {
    const of = this.host.recurrences(r);
    if (of === undefined) throw new Error(`internal: table "${r.name}" names a recurrence that src/map/recur.ts does not know`);
    const { state, part } = of;
    if (part !== "kept") { d.words.push(Fill.Recurrence, ...head, PARTS.indexOf(part), this.kernel(state.table)); return; }
    this.binder(state.index.id, `K${k}i`);
    this.binder(state.clock.id, `K${k}t`);
    const start = state.startLanes.map((x) => this.visit(x.id)), step = state.stepLanes.map((x) => this.visit(x.id));
    const cells = state.shape.kind === "list" ? [0, state.shape.n] : [1, state.shape.w, state.shape.h];
    const every = state.kept.kind === "every" ? 1 : state.kept.every;
    d.words.push(Fill.Recurrence, ...head, 0, ...cells, this.lit(state.rate), this.lit(state.from), this.lit(state.until), state.first, state.steps, every,
      this.kernel(state.prev), this.outOf(state.index.id), this.outOf(state.clock.id));
    d.roots.push(...start, ...step);
  }
  /** An author compute pass (src/compute.ts): a Kernel decl with no table head, reached through its readers. */
  private compute(p: ComputePass): number {
    const seen = this.declOf.get(p);
    if (seen !== undefined) return seen;
    const { k, d } = this.reserve(Decl.Kernel, p);
    const w = p.when, when = w.kind === "steps" ? [WHEN.steps, this.sym(w.start), w.count, this.lit(w.rate), w.keep] : [WHEN[w.kind]];
    const libs = p.code.libs.map((l) => this.rawFn(l)), values = p.code.values.map((x) => this.visit(x.id));
    const writes = p.writes.flatMap(([key, r]) => [this.sym(key), this.sym(r.symbol), ...(r.kind === "buffer"
      ? [0, r.entries, r.lanes]
      : [1, r.size.length, r.size[0], r.size[1], r.size[2] ?? 1, this.sym(r.format)])]);
    d.words.push(Fill.Compute, this.sym(p.name), ...when, this.sym(p.entry), ...p.dispatch, this.sym(p.code.text), libs.length, ...libs, p.writes.length, ...writes);
    d.roots.push(...values);
    return k;
  }
  /** An offscreen pass (src/frame/pass.ts) a raw function reads: its table camera, then a Pass decl targeting its texture. */
  private offscreen(def: PassDef): number {
    const seen = this.declOf.get(def);
    if (seen !== undefined) return seen;
    const what = def.what;
    switch (what.kind) {
      case "screen": throw new Unrecorded(`the screen pass "${def.name}": the wire has no layout for a pass drawn by a colour over its uv yet`);
      case "scene": {
        const kernel = this.kernel(what.camera);
        const { k: camera, d: cam } = this.reserve(Decl.Camera, what.camera);
        cam.words.push(this.sym(def.name), 2, kernel);
        const draws = this.count.get(Decl.Draw) ?? 0, size = def.size.kind === "view" ? [0, this.lit(def.size.scale)] : [1, def.size.width, def.size.height];
        const { k, d } = this.reserve(Decl.Pass, def);
        // Clear is written load: what an offscreen texture starts as is its keep's (the host's lowering).
        d.words.push(camera, draws, draws, 0, 0, 0, 1, this.sym(def.name), ...size, KEEP[what.keep], 1);
        return k;
      }
    }
  }
  private read(r: LibRead): number[] {
    switch (r.kind) {
      case "table": return [0, this.kernel(r.table)];
      case "compute": return [1, this.compute(r.pass), r.pass.writes.findIndex(([key]) => key === r.key)];
      case "pass": return [2, this.offscreen(r.pass)];
    }
  }

  // ---- scene-level decls (src/record/decls.ts; layouts in loft-ir wire.rs) ----

  private lit(...xs: number[]): number { return this.lits.pushAll(xs); }
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
      case "grid": return [0, c.u, c.v, this.role(c.place), this.role(c.normal), this.role(c.cells), c.count];
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
    d.words.push(this.sym(x.name), x.layer === "subject" ? 0 : 1, ["none", "back", "front"].indexOf(x.raster.cull), Number(x.raster.depthWrite), blend, paint, ...cover);
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
    d.words.push(camera, first, end, ...clear, ...ground, ...outline, 0);
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
    for (const u of s.unknowns) d.words.push(this.sym(u.name), u.numbers, ...u.jets.map((x) => this.outOf(x.id)));
    d.words.push(s.near.length, ...s.near.map((n) => n.unknown), s.holds.length);
    for (const h of s.holds) {
      d.words.push(...(h.kind === "equal" ? [0, this.lit(h.within)] : [1]));
      d.words.push(...(h.times.kind === "at" ? [0, this.lit(h.times.t)] : h.times.kind === "during" ? [1, this.lit(h.times.from, h.times.to)] : [2]));
      d.words.push(this.sym(h.said), this.sym(h.site));
    }
    d.roots.push(...from, ...to, ...margins);
  }

  private rawFn(f: LibFn): number {
    const seen = this.declOf.get(f);
    if (seen !== undefined) return seen;
    const deps = (f.deps ?? []).map((x) => this.rawFn(x)), reads = f.reads ?? [];
    const words = reads.flatMap((r) => this.read(r));
    const { k, d } = this.reserve(Decl.Fn, f);
    d.words.push(1, this.sym(f.name), TY[f.ret], this.sym(f.wgsl), deps.length, ...deps, reads.length, ...words);
    return k;
  }
  private graphFn(fn: number, f: Lowered & { kind: "body" }): number {
    const key = this.log.fns[fn]!, seen = this.declOf.get(key);
    if (seen !== undefined) return seen;
    const { k, d } = this.reserve(Decl.Fn, key);
    f.binders.forEach((b, j) => this.binder(b, `F${k}a${j}`));
    d.roots.push(this.visit(f.body));
    d.words.push(0, this.sym(key.name), TY[f.ret], f.binders.length, ...f.binders.map((b) => this.outOf(b)));
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
    d.words.push(l.init.length, stop.length, ...l.binders.map((b) => this.outOf(b)));
    return k;
  }
}

/** The process's recorder: src/expr.ts appends to it. */
export const rec = new Recorder();
