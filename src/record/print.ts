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
// After the roots line, one line per table in the order the walk first fetched it (a fill's own fetches append to the
// list), its fill walked after the roots; a fill's binders print as `fill TY SLOT` (one level in):
//   kernel TABLE map COUNT [#lanes...] COUNT [...]...      kernel TABLE scan FIRST STEPS [#init...] -> [#next...] [#lanes...]
//   kernel TABLE data SAMPLEHASH                           kernel TABLE recurrence NAME
// where TABLE is table:NAME:LANES:ENTRIES:ROLE as in a fetch (rows:LABEL for a curve). Then one line per scene-level
// decl (cameras, media, draws, passes, checks, solves, each in decl order), walked after the roots and before the fills;
// a graph function a decl names prints as `#body` over binders `arg TY K`, a solve's jets as `jet TY UNKNOWN ORDER`:
//   camera NAME look [#eye #target #up #fovy] | matrix [#c0 #c1 #c2 #c3]
//   medium march #density within HEX steps STEPS SHADOW | medium gaussians TABLE
//   draw NAME subject|surroundings CULL DEPTH BLEND paint #p COVER, COVER one of grid U V #place COUNT,
//        mesh TABLE COUNT, quads #place COUNT, ray box HEX|instances TABLE HEX hit #h|medium K, screen
//   pass camera K draws FIRST END clear load|HEX ground none|#hit #paint outline none|#ink
//   check LABEL prove BOXES|sample N FORMULA, FORMULA one of (atom #m #t), (always|eventually [#leaf #lo #hi, ...] F),
//        (and|or F F...), (not F)
//   solve LABEL HEX unknowns [NAME N #from, ...] near [K #to, ...] holds [equal #m HEX|bound #m at HEX|during HEX|span SAID SITE, ...]
// TEXTHASH and SAMPLEHASH are FNV-1a (32 bit, hex; each word xor-ed whole) over the text's UTF-16 units and the f32 bits.

import { variable, type Expr, type Loop, type NumTy, type TableSource, type Ty } from "../expr.ts";
import { BOUND, Decl, declsOf, Fill, Op, PRIM0, PRIMS, ROLES, swizzleSel, symOf, TYS, type Job } from "./wire.ts";

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
  text(roots: readonly number[], kernels: readonly string[]): string { return [...this.lines, `roots ${roots.map((k) => `#${k}`).join(" ")}`, ...kernels].join("\n"); }
  /** A fill's scope: one level in, its binders (node id, type, slot) bound to `fill TY SLOT` lines. */
  fill(binders: readonly (readonly [number, Ty, string])[]): Scope { return this.scoped("fill", binders); }
  /** A scope one level in whose binders print as `WORD TY SLOT`. */
  scoped(word: string, binders: readonly (readonly [number, Ty, string])[]): Scope {
    const s = inner(root());
    for (const [id, ty, slot] of binders) s.bound.set(id, this.line(`${word} ${ty} ${slot}`, 1));
    return s;
  }
}
const refs = (ks: readonly number[]): string => `[${ks.map((k) => `#${k}`).join(" ")}]`;
const tableName = (name: string, lanes: number, entries: number, role: string): string => `table:${name}:${lanes}:${entries}:${role}`;

/**
 * Where a walk is: the loop level, the binders it reads (node id -> canonical line), the call frame it is in, and its
 * memo. A loop scope reads through to its parent's memo; a call scope does not (its body's binders mean that call's
 * arguments). A value that reads no binder is memoised for the whole walk.
 */
type Scope = { readonly level: number; readonly frame: number; readonly bound: Map<number, number>; readonly args: ReadonlyMap<number, () => number>; readonly memo: Map<number, number>; readonly parent: Scope | undefined };
const root = (): Scope => ({ level: 0, frame: 0, bound: new Map(), args: new Map(), memo: new Map(), parent: undefined });
const inner = (s: Scope): Scope => ({ level: s.level + 1, frame: s.frame, bound: new Map(s.bound), args: s.args, memo: new Map(), parent: s });
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
      case "fetch": {
        const k = c.op(`fetch ${tableName(n.table.name, n.table.lanes, n.table.entries, n.table.role)} ${n.lane}`, kids([n.index]));
        if (!tables.includes(n.table)) tables.push(n.table);
        return k;
      }
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
  const tables: TableSource[] = [];
  const top = roots.map((r) => walk(r, root()));
  const kernels: string[] = [];
  // Each table's fill, over fresh variables bound as its binders (`tables` grows while its fills are walked).
  for (let j = 0; j < tables.length; j++) {
    const t = tables[j]!, head = `kernel ${tableName(t.name, t.lanes, t.entries, t.role)}`;
    if ("data" in t) { kernels.push(`${head} data ${samples(t.data)}`); continue; }
    if ("recurrence" in t) { kernels.push(`${head} recurrence ${t.recurrence}`); continue; }
    const p = t.program, fresh = <T extends NumTy>(slot: string, ty: T): Expr<T> => variable(`print:${slot}@${t.name}`, ty);
    switch (p.kind) {
      case "map": {
        const runs = p.runs.map((run, r) => {
          const i = fresh(`r${r}i`, "f32"), s = c.fill([[i.id, "f32", "i"]]);
          return `${run.count} ${refs(run.lanes(i).map((x) => walk(x, s)))}`;
        });
        kernels.push(`${head} map ${runs.join(" ")}`);
        break;
      }
      case "scan": {
        const init = p.init.map((x) => walk(x, root()));
        const state = p.init.map((x, k) => fresh(`s${k}`, x.ty as NumTy)), i = fresh("i", "f32");
        const s = c.fill([...state.map((x, k) => [x.id, x.ty, `s${k}`] as const), [i.id, "f32", "i"]]);
        const step = p.step(state, i);
        kernels.push(`${head} scan ${p.first} ${p.steps} ${refs(init)} -> ${refs(step.next.map((x) => walk(x, s)))} ${refs(step.lanes.map((x) => walk(x, s)))}`);
        break;
      }
    }
  }
  return c.text(top, kernels);
}

/** The canonical print of a job: its program roots, with every call inlined. */
export function printJob(job: Job): string {
  const c = new Canon(), global = new Map<number, number>(), { programRoots, decls } = declsOf(job);
  const of = (tag: Decl) => decls.filter((d) => d.tag === tag);
  const fns = of(Decl.Fn), loops = of(Decl.Loop), kernels = of(Decl.Kernel), params = of(Decl.Param), inputs = of(Decl.Input);
  const N = job.nodes, sym = (k: number): string => symOf(job, k);
  let frames = 0; // call frames entered, so each call's body has its own loop lines
  const walk = (id: number, s: Scope, inCall: boolean): number => {
    const hit = s.bound.get(id) ?? s.args.get(id)?.() ?? find(s, global, id);
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
        const w = kernels[imm]!.words, rows = w[0] === Fill.Rows;
        const res = rows ? `rows:${sym(w[1]!)}:${samples(job.rows.subarray(w[2]!, w[2]! + w[3]!))}` : tableName(sym(w[1]!), w[2]!, w[3]!, ROLES[w[4]!]!);
        const k = c.op(`fetch ${res} ${flags}`, kids());
        if (!rows && !tables.includes(imm)) tables.push(imm);
        return k;
      }
      case Op.Raw: { const w = fns[imm]!.words; return c.op(`raw ${sym(w[1]!)} ${ty} ${fnv(codes(sym(w[3]!)))}`, kids()); }
      case Op.Swizzle: return c.op(`get ${ty} ${swizzleSel(imm)}`, kids());
      case Op.Vec: return c.op(`vec ${ty}`, kids());
      case Op.Loop: return c.op(`iter ${ty} ${flags}`, [loop(imm, s, inCall)]);
      case Op.Call: {
        const d = fns[imm]!, args = job.args.subarray(at, at + n);
        // A call's body sees only its binders, bound to this call's arguments, at the caller's level. An argument is
        // walked where its binder is first read, not before the body: the TS graph, which inlined the call, meets it there.
        const body: Scope = { level: s.level, frame: ++frames, bound: new Map(), args: new Map(Array.from(d.words.subarray(4, 4 + d.words[3]!), (b, j) => [b, () => walk(args[j]!, s, inCall)])), memo: new Map(), parent: undefined };
        return walk(d.roots[0]!, body, true);
      }
      case Op.Grad: throw new Error("job: a Grad node (the wave-1 recorder writes derivatives expanded)");
    }
  };
  // Per (loop, level, call frame, enclosing binders): its line, so its K results share one walk. The frame matters
  // because call arguments are not in `bound`: a loop in a graph function that reads an argument differs per call.
  const loopLine = new Map<string, number>();
  const loop = (k: number, s: Scope, inCall: boolean): number => {
    const d = loops[k]!, nState = d.words[0]!, hasStop = d.words[1]!, key = `${k}:${s.level}:${s.frame}:${[...s.bound.values()].join(",")}`;
    const seen = loopLine.get(key);
    if (seen !== undefined) return seen;
    const r = d.roots, count = walk(r[0]!, s, inCall), init = Array.from(r.subarray(1, 1 + nState), (x) => walk(x, s, inCall)), body = inner(s);
    Array.from(d.words.subarray(2, 3 + nState)).forEach((b, j) => body.bound.set(b, c.line(`bind ${TYS[(N[4 * b]! >> 8) & 0xff]} ${s.level} ${j === nState ? "i" : `s${j}`}`, 1 << s.level)));
    const next = Array.from(r.subarray(1 + nState, 1 + 2 * nState), (x) => walk(x, body, inCall));
    const out = c.loop(s.level, count, init, next, hasStop ? walk(r[1 + 2 * nState]!, body, inCall) : undefined);
    loopLine.set(key, out);
    return out;
  };
  const tables: number[] = [];
  const top = Array.from(programRoots, (r) => walk(r, root(), false));
  const out: string[] = [];
  const tyOf = (b: number): Ty => TYS[(N[4 * b]! >> 8) & 0xff]!;
  const scene = sceneLines(job, decls, (x, s) => walk(x, s ?? root(), false), (word, b) => c.scoped(word, b.map(([id, slot]) => [id, tyOf(id), slot] as const)));
  for (let j = 0; j < tables.length; j++) {
    const d = kernels[tables[j]!]!, w = d.words, r = d.roots, lanes = w[2]!;
    const head = `kernel ${tableName(sym(w[1]!), lanes, w[3]!, ROLES[w[4]!]!)}`;
    switch (w[0]! as Fill) {
      case Fill.Map: {
        const runs = Array.from({ length: w[5]! }, (_, k) => {
          const s = c.fill([[w[7 + 2 * k]!, "f32", "i"]]);
          return `${w[6 + 2 * k]} ${refs(Array.from(r.subarray(k * lanes, (k + 1) * lanes), (x) => walk(x, s, false)))}`;
        });
        out.push(`${head} map ${runs.join(" ")}`);
        break;
      }
      case Fill.Scan: {
        const n = w[7]!, binders = Array.from(w.subarray(8, 9 + n));
        const init = Array.from(r.subarray(0, n), (x) => walk(x, root(), false));
        const s = c.fill(binders.map((b, k) => [b, tyOf(b), k === n ? "i" : `s${k}`] as const));
        const next = Array.from(r.subarray(n, 2 * n), (x) => walk(x, s, false)), ls = Array.from(r.subarray(2 * n, 2 * n + lanes), (x) => walk(x, s, false));
        out.push(`${head} scan ${w[5]} ${w[6]} ${refs(init)} -> ${refs(next)} ${refs(ls)}`);
        break;
      }
      case Fill.Data: out.push(`${head} data ${samples(job.rows.subarray(w[5]!, w[5]! + lanes * w[3]! * 4))}`); break;
      case Fill.Recurrence: out.push(`${head} recurrence ${sym(w[5]!)}`); break;
      case Fill.Rows: throw new Error("internal: a rows kernel is not a table");
    }
  }
  return c.text(top, [...out, ...scene]);
}

type Walk = (node: number, s?: Scope) => number;
type Scoped = (word: string, binders: readonly (readonly [number, string])[]) => Scope;

/** The scene-level decls' lines (print.ts header; loft-ir print.rs `decls`), walked in this order. */
function sceneLines(job: Job, decls: ReturnType<typeof declsOf>["decls"], walk: Walk, scoped: Scoped): string[] {
  const of = (tag: Decl) => decls.filter((d) => d.tag === tag), fns = of(Decl.Fn), kernels = of(Decl.Kernel);
  const sym = (k: number): string => symOf(job, k), lits = (at: number, n: number): string => hex(job.lits.subarray(at, at + n));
  const bare = (xs: ArrayLike<number>, s?: Scope): string => Array.from(xs, (x) => `#${walk(x, s)}`).join(" ");
  const refs = (xs: ArrayLike<number>, s?: Scope): string => `[${bare(xs, s)}]`;
  const fnRef = (ord: number): string => {
    const d = fns[ord]!, binders = Array.from(d.words.subarray(4, 4 + d.words[3]!), (b, k) => [b, String(k)] as const);
    return `#${walk(d.roots[0]!, scoped("arg", binders))}`;
  };
  const table = (k: number): string => {
    const w = kernels[k]!.words;
    return w[0] === Fill.Rows ? `rows:${sym(w[1]!)}` : tableName(sym(w[1]!), w[2]!, w[3]!, ROLES[w[4]!]!);
  };
  const lines: string[] = [];
  for (const d of of(Decl.Camera)) lines.push(`camera ${sym(d.words[0]!)} ${d.words[1] === 0 ? "look" : "matrix"} ${refs(d.roots)}`);
  for (const d of of(Decl.Medium)) {
    const w = d.words;
    lines.push(w[0] === 0 ? `medium march ${fnRef(w[1]!)} within ${lits(w[2]!, 6)} steps ${w[3]} ${w[4]}` : `medium gaussians ${table(w[1]!)}`);
  }
  for (const d of of(Decl.Draw)) {
    const w = d.words, paint = fnRef(w[5]!), c = w.subarray(6);
    const find = (at: number): string => (c[at] === 0 ? `hit ${fnRef(c[at + 1]!)}` : `medium ${c[at + 1]}`);
    const cover = c[0] === 0 ? `grid ${c[1]} ${c[2]} ${fnRef(c[3]!)} ${c[4]}`
      : c[0] === 1 ? `mesh ${table(c[1]!)} ${c[2]}`
      : c[0] === 2 ? `quads ${fnRef(c[1]!)} ${c[2]}`
      : c[0] === 3 ? `ray box ${lits(c[3]!, 6)} ${find(1)}`
      : c[0] === 4 ? `ray instances ${table(c[3]!)} ${lits(c[4]!, 6)} ${find(1)}`
      : "screen";
    lines.push(`draw ${sym(w[0]!)} ${w[1] === 0 ? "subject" : "surroundings"} ${w[2]} ${w[3]} ${["opaque", "alpha", "add"][w[4]!]} paint ${paint} ${cover}`);
  }
  for (const d of of(Decl.Pass)) {
    const w = d.words;
    let at = 3;
    const clear = w[at++] === 0 ? "load" : lits(w[at++]!, 4);
    const ground = w[at++] === 0 ? "none" : `${fnRef(w[at++]!)} ${fnRef(w[at++]!)}`;
    const outline = w[at++] === 0 ? "none" : fnRef(w[at++]!);
    lines.push(`pass camera ${w[0]} draws ${w[1]} ${w[2]} clear ${clear} ground ${ground} outline ${outline}`);
  }
  for (const d of of(Decl.Check)) {
    const w = d.words;
    let at = 3, r = 0;
    const formula = (): string => {
      const k = w[at++]!;
      switch (k) {
        case 0: { const m = d.roots[r++]!, t = d.roots[r++]!; return `(atom ${bare([m, t])})`; }
        case 1: case 2: {
          const n = w[at++]!, leaves = Array.from(w.subarray(at, at + n));
          at += n;
          const lo = Array.from(d.roots.subarray(r, r + n)), hi = Array.from(d.roots.subarray(r + n, r + 2 * n));
          r += 2 * n;
          const ls = leaves.map((l, j) => bare([l, lo[j]!, hi[j]!]));
          return `(${k === 1 ? "always" : "eventually"} [${ls.join(", ")}] ${formula()})`;
        }
        case 3: case 4: { const n = w[at++]!; return `(${k === 3 ? "and" : "or"} ${Array.from({ length: n }, formula).join(" ")})`; }
        default: return `(not ${formula()})`;
      }
    };
    const body = formula();
    lines.push(`check ${sym(w[0]!)} ${w[1] === 0 ? "prove" : "sample"} ${w[2]} ${body}`);
  }
  for (const d of of(Decl.Solve)) {
    const w = d.words, n = w[2]!, unknowns: string[] = [], binders: [number, string][] = [];
    let at = 3, r = 0;
    for (let u = 0; u < n; u++) {
      unknowns.push(`${sym(w[at]!)} ${w[at + 1]}`);
      for (let o = 0; o < 4; o++) binders.push([w[at + 2 + o]!, `${u} ${o}`]);
      at += 6;
    }
    const m = w[at++]!, nearOf = Array.from(w.subarray(at, at + m));
    at += m;
    const h = w[at++]!, holds: { kind: string; times: string; said: string; site: string }[] = [];
    for (let k = 0; k < h; k++) {
      const kind = w[at++] === 0 ? `equal ${lits(w[at++]!, 1)}` : "bound";
      const t = w[at++]!, times = t === 0 ? `at ${lits(w[at++]!, 1)}` : t === 1 ? `during ${lits(w[at++]!, 2)}` : "span";
      holds.push({ kind, times, said: sym(w[at++]!), site: sym(w[at++]!) });
    }
    const us = unknowns.map((u) => `${u} #${walk(d.roots[r++]!)}`);
    const near = nearOf.map((k) => `${k} #${walk(d.roots[r++]!)}`);
    const s = scoped("jet", binders);
    const hs = holds.map((x) => {
      const margin = `#${walk(d.roots[r++]!, s)}`, [word, ...within] = x.kind.split(" ");
      return `${word} ${margin}${within.length > 0 ? ` ${within.join(" ")}` : ""} ${x.times} ${x.said} ${x.site}`;
    });
    lines.push(`solve ${sym(w[0]!)} ${lits(w[1]!, 6)} unknowns [${us.join(", ")}] near [${near.join(", ")}] holds [${hs.join(", ")}]`);
  }
  return lines;
}
