// The canonical print: the one text both sides of the shadow comparison produce (section 8.2, lane 1.2). `printGraph`
// reads the TS engine's graph (`Expr.node`, never the recorder); `printJob` reads a job's spans and inlines its calls.
// loft-ir's printer must produce the same text from the same job, byte for byte.
//
// The print is the graph after hash-consing: one line per distinct node, numbered in first-use order of a post-order
// walk from the roots (arguments in order; a loop's count, start, then its body), then a line of the roots.
//   lit TY HEX...             f64 bits of each component        time
//   param NAME HEX HEX HEX    def, min, max                     input TY NAME HEX...
//   var TY NAME               a free variable                   bind TY LEVEL SLOT     a loop binder (slot s0.. or i)
//   PRIM TY #a...             vec TY #a...    get TY SEL #a     raw NAME TY TEXTHASH #a...
//   fetch table:NAME:LANES:ENTRIES:ROLE LANE #i                 fetch rows:LABEL:SAMPLEHASH 0 #x
//   loop #count [#init...] -> [#next...] until #stop|-          iter TY K #loop
// A loop's body is walked one level deeper: its binders print as `bind TY LEVEL SLOT`, LEVEL counting the loops it is
// inside, so two equal loops are one node. A call's body is walked with its binders read as the call's arguments.
// TEXTHASH and SAMPLEHASH are FNV-1a (32 bit, hex; each word xor-ed whole) over the text's UTF-16 units and the f32 bits.

import type { Expr, Loop, Ty } from "../expr.ts";
import { BOUND, Decl, declsOf, Op, PRIM0, PRIMS, ROLES, swizzleSel, symOf, TYS, type Job } from "./wire.ts";

const fnv = (xs: ArrayLike<number>): string => {
  let h = 0x811c9dc5;
  for (let i = 0; i < xs.length; i++) h = Math.imul(h ^ xs[i]!, 0x01000193);
  return (h >>> 0).toString(16).padStart(8, "0");
};
const codes = (s: string): number[] => Array.from(s, (c) => c.charCodeAt(0));
const f64 = new Float64Array(1), u64 = new BigUint64Array(f64.buffer);
const hex = (xs: ArrayLike<number>): string => Array.from(xs, (x) => { f64[0] = x; return u64[0]!.toString(16); }).join(" ");
const samples = (xs: Float32Array): string => fnv(new Uint32Array(xs.buffer, xs.byteOffset, xs.length));

/** The interned lines, and per line the loop levels its value reads (bit L: a binder of a loop at level L). */
class Canon {
  readonly lines: string[] = [];
  readonly levels: number[] = [];
  private readonly ids = new Map<string, number>();
  line(text: string, levels: number): number {
    let k = this.ids.get(text);
    if (k === undefined) { this.ids.set(text, (k = this.lines.length)); this.lines.push(text); this.levels.push(levels); }
    return k;
  }
  op(text: string, kids: readonly number[]): number {
    return this.line(`${text} ${kids.map((k) => `#${k}`).join(" ")}`.trimEnd(), kids.reduce((m, k) => m | this.levels[k]!, 0));
  }
  loop(level: number, count: number, init: readonly number[], next: readonly number[], stop: number | undefined): number {
    const outer = [count, ...init].reduce((m, k) => m | this.levels[k]!, 0) | ([...next, ...(stop === undefined ? [] : [stop])].reduce((m, k) => m | this.levels[k]!, 0) & ~(1 << level));
    const r = (xs: readonly number[]): string => xs.map((k) => `#${k}`).join(" ");
    return this.line(`loop #${count} [${r(init)}] -> [${r(next)}] until ${stop === undefined ? "-" : `#${stop}`}`, outer);
  }
  text(roots: readonly number[]): string { return [...this.lines, `roots ${roots.map((k) => `#${k}`).join(" ")}`].join("\n"); }
}

/**
 * Where a walk is: the loop level, the binders it reads (node id -> canonical line), and its memo. A loop scope reads
 * through to its parent's memo; a call scope does not (its body's binders mean that call's arguments). A value that
 * reads no binder is memoised for the whole walk.
 */
type Scope = { readonly level: number; readonly bound: Map<number, number>; readonly memo: Map<number, number>; readonly parent: Scope | undefined };
const root = (): Scope => ({ level: 0, bound: new Map(), memo: new Map(), parent: undefined });
const inner = (s: Scope): Scope => ({ level: s.level + 1, bound: new Map(s.bound), memo: new Map(), parent: s });
function find(s: Scope, global: Map<number, number>, id: number): number | undefined {
  const g = global.get(id);
  if (g !== undefined) return g;
  for (let x: Scope | undefined = s; x !== undefined; x = x.parent) { const m = x.memo.get(id); if (m !== undefined) return m; }
  return undefined;
}

/** The canonical print of the TS graph from `roots`. */
export function printGraph(roots: readonly Expr[]): string {
  const c = new Canon(), global = new Map<number, number>();
  const loopLine = new Map<string, number>(); // per (loop, level): its line, so its K results share one walk
  const walk = (e: Expr, s: Scope): number => {
    const hit = s.bound.get(e.id) ?? find(s, global, e.id);
    if (hit !== undefined) return hit;
    const k = node(e, s);
    (c.levels[k] === 0 ? global : s.memo).set(e.id, k);
    return k;
  };
  const node = (e: Expr, s: Scope): number => {
    const n = e.node, kids = (xs: readonly Expr[]): number[] => xs.map((x) => walk(x, s));
    switch (n.op) {
      case "const": return c.line(`lit ${n.ty} ${hex(n.v)}`, 0);
      case "var": return c.line(`var ${n.ty} ${n.name}`, 0);
      case "input": return n.spec.name === "time" ? c.line("time", 0) : c.line(`input ${n.ty} ${n.spec.name} ${hex(n.spec.def)}`, 0);
      case "param": return c.line(`param ${n.spec.name} ${hex([n.spec.def, n.spec.min, n.spec.max])}`, 0);
      case "row": return c.op(`fetch rows:${n.row.label}:${samples(n.row.samples)} 0`, kids([n.x]));
      case "fetch": return c.op(`fetch table:${n.table.name}:${n.table.lanes}:${n.table.entries}:${n.table.role} ${n.lane}`, kids([n.index]));
      case "call": return c.op(`${n.fn} ${n.ty}`, kids(n.args));
      case "vec": return c.op(`vec ${n.ty}`, kids(n.args));
      case "get": return c.op(`get ${n.ty} ${n.sel}`, kids([n.a]));
      case "lib": return c.op(`raw ${n.fn.name} ${n.ty} ${fnv(codes(n.fn.wgsl))}`, kids(n.args));
      case "iterate": return c.op(`iter ${n.ty} ${n.k}`, [loop(n.loop, s)]);
    }
  };
  const loop = (l: Loop, s: Scope): number => {
    const key = `${l.id}:${s.level}`, seen = loopLine.get(key);
    if (seen !== undefined) return seen;
    const count = walk(l.count, s), init = l.init.map((x) => walk(x, s)), body = inner(s);
    [...l.state, l.index].forEach((b, j) => body.bound.set(b.id, c.line(`bind ${b.ty} ${s.level} ${j === l.state.length ? "i" : `s${j}`}`, 1 << s.level)));
    const k = c.loop(s.level, count, init, l.next.map((x) => walk(x, body)), l.stop === undefined ? undefined : walk(l.stop, body));
    loopLine.set(key, k);
    return k;
  };
  return c.text(roots.map((r) => walk(r, root())));
}

/** The canonical print of a job: its program roots, with every call inlined. */
export function printJob(job: Job): string {
  const c = new Canon(), global = new Map<number, number>(), { programRoots, decls } = declsOf(job);
  const of = (tag: Decl) => decls.filter((d) => d.tag === tag);
  const fns = of(Decl.Fn), loops = of(Decl.Loop), kernels = of(Decl.Kernel), params = of(Decl.Param), inputs = of(Decl.Input);
  const N = job.nodes, sym = (k: number): string => symOf(job, k);
  const walk = (id: number, s: Scope, inCall: boolean): number => {
    const hit = s.bound.get(id) ?? find(s, global, id);
    if (hit !== undefined) return hit;
    const k = node(id, s, inCall);
    (c.levels[k] === 0 && !inCall ? global : s.memo).set(id, k);
    return k;
  };
  const node = (id: number, s: Scope, inCall: boolean): number => {
    const h = N[4 * id]!, at = N[4 * id + 1]!, n = N[4 * id + 2]!, imm = N[4 * id + 3]!;
    const op = h & 0xff, ty: Ty = TYS[(h >> 8) & 0xff]!, flags = h >>> 16;
    const kids = (): number[] => Array.from(job.args.subarray(at, at + n), (a) => walk(a, s, inCall));
    if (op >= PRIM0) return c.op(`${PRIMS[op - PRIM0]} ${ty}`, kids());
    switch (op as Op) {
      case Op.Lit: return c.line(`lit ${ty} ${hex(job.lits.subarray(imm, imm + n))}`, 0);
      case Op.Time: return c.line("time", 0);
      case Op.Param: { const w = params[imm]!.words; return c.line(`param ${sym(w[0]!)} ${hex([job.lits[w[1]!]!, job.lits[w[2]!]!, job.lits[w[3]!]!])}`, 0); }
      case Op.Input: { const w = inputs[imm]!.words, size = ty === "f32" ? 1 : Number(ty.slice(3)); return c.line(`input ${ty} ${sym(w[0]!)} ${hex(job.lits.subarray(w[2]!, w[2]! + size))}`, 0); }
      case Op.Bind:
        if (flags === BOUND) throw new Error(`job: binder #${id} (${sym(imm)}) read outside its loop or function`);
        return c.line(`var ${ty} ${sym(imm)}`, 0);
      case Op.Fetch: {
        const w = kernels[imm]!.words;
        const res = w[0] === 1 ? `rows:${sym(w[1]!)}:${samples(job.rows.subarray(w[2]!, w[2]! + w[3]!))}` : `table:${sym(w[1]!)}:${w[2]}:${w[3]}:${ROLES[w[4]!]}`;
        return c.op(`fetch ${res} ${flags}`, kids());
      }
      case Op.Raw: { const w = fns[imm]!.words; return c.op(`raw ${sym(w[1]!)} ${ty} ${fnv(codes(sym(w[3]!)))}`, kids()); }
      case Op.Swizzle: return c.op(`get ${ty} ${swizzleSel(imm)}`, kids());
      case Op.Vec: return c.op(`vec ${ty}`, kids());
      case Op.Loop: return c.op(`iter ${ty} ${flags}`, [loop(imm, s, inCall)]);
      case Op.Call: {
        const d = fns[imm]!, args = kids();
        // A call's body sees only its binders, bound to this call's arguments, at the caller's level.
        const body: Scope = { level: s.level, bound: new Map(Array.from(d.words.subarray(4, 4 + d.words[3]!), (b, j) => [b, args[j]!])), memo: new Map(), parent: undefined };
        return walk(d.roots[0]!, body, true);
      }
      case Op.Grad: throw new Error("job: a Grad node (the wave-1 recorder writes derivatives expanded)");
    }
  };
  const loopLine = new Map<string, number>();
  const loop = (k: number, s: Scope, inCall: boolean): number => {
    const d = loops[k]!, nState = d.words[0]!, hasStop = d.words[1]!, key = `${k}:${s.level}:${[...s.bound.values()].join(",")}`;
    const seen = loopLine.get(key);
    if (seen !== undefined) return seen;
    const r = d.roots, count = walk(r[0]!, s, inCall), init = Array.from(r.subarray(1, 1 + nState), (x) => walk(x, s, inCall)), body = inner(s);
    Array.from(d.words.subarray(2, 3 + nState)).forEach((b, j) => body.bound.set(b, c.line(`bind ${TYS[(N[4 * b]! >> 8) & 0xff]} ${s.level} ${j === nState ? "i" : `s${j}`}`, 1 << s.level)));
    const next = Array.from(r.subarray(1 + nState, 1 + 2 * nState), (x) => walk(x, body, inCall));
    const out = c.loop(s.level, count, init, next, hasStop ? walk(r[1 + 2 * nState]!, body, inCall) : undefined);
    loopLine.set(key, out);
    return out;
  };
  return c.text(Array.from(programRoots, (r) => walk(r, root(), false)));
}
