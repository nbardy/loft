// The TS graph -> loft-core `Ssa` records walk shared by the lane 2.3 bridges (dump_checks.ts, dump_solves.ts), until
// the wire carries Check and Solve decls (§3.2). A fetch at a constant entry is inlined from its table program, as
// `src/interval.ts` does; a constant-count loop is unrolled. `solve`'s jet `JET(time, v_d, ...)` is its value v_d.
import * as E from "../../../src/expr.ts";
import type { Expr, Fn, TableSource } from "../../../src/expr.ts";

const FNS: Fn[] = [
  "add", "sub", "mul", "div", "neg", "min", "max", "pow", "mod", "atan2", "step",
  "abs", "sign", "floor", "fract", "sqrt", "exp", "log", "sin", "cos", "tan", "saturate", "normalize",
  "mix", "clamp", "smoothstep", "length", "distance", "dot", "cross",
  "lt", "gt", "le", "ge", "eq", "ne", "and", "or", "not", "select",
  "ceil", "trunc", "roundEven", "rem", "inverseSqrt", "exp2", "log2", "asin", "acos", "atan", "sinh", "cosh", "tanh",
];
const SEL = "xyzw";
const constant = (e: Expr): number => E.compileJs([e])({ vars: {}, inputs: {}, params: {} })[0] as number;

/** One graph: `slot(name, held)` names a leaf slot, `walk(e)` records e and returns its node index. */
export function graph(name: string) {
  const slots: string[] = [], held: number[] = [], nodes: unknown[] = [], at = new Map<number, number>();
  const slot = (s: string, v: number): number => {
    if (!slots.includes(s)) { slots.push(s); held.push(Math.fround(v)); }
    return slots.indexOf(s);
  };
  const entry = (t: TableSource, k: number): readonly Expr[] => {
    if (!("program" in t)) throw new Error(`${name}: table ${t.name} has no program`);
    const p = t.program;
    switch (p.kind) {
      case "map": { let s = 0; for (const r of p.runs) { if (k < s + r.count) return r.lanes(E.f(k - s)); s += r.count; } throw new Error("past the table"); }
      case "scan": {
        let state = p.init;
        for (let j = 0; j < p.steps; j++) { const s = p.step(state, E.f(j)); if (p.first + j === k) return s.lanes; state = s.next; }
        throw new Error("outside the scan");
      }
    }
  };
  const alias = (e: Expr, inner: Expr): number => { const id = walk(inner); at.set(e.id, id); return id; };
  const walk = (e: Expr): number => {
    const seen = at.get(e.id); if (seen !== undefined) return seen;
    const n = e.node;
    let rec: unknown;
    switch (n.op) {
      case "const": rec = { k: "lit", v: n.v.map(Number) }; break;
      case "var": rec = { k: "leaf", slot: slot(n.name, 0) }; break;
      case "param": rec = { k: "leaf", slot: slot(n.spec.name, n.spec.def) }; break;
      case "input": if (n.spec.def.length !== 1) throw new Error(`${name}: vector input`); rec = { k: "leaf", slot: slot(n.spec.name, n.spec.def[0]!) }; break;
      case "call": rec = { k: "call", fn: FNS.indexOf(n.fn), args: n.args.map(walk) }; break;
      case "vec": rec = { k: "vec", w: { vec2: 2, vec3: 3, vec4: 4 }[n.ty], args: n.args.map(walk) }; break;
      case "get": rec = { k: "get", a: walk(n.a), sel: [...n.sel].map((ch) => SEL.indexOf(ch)) }; break;
      case "lib": if (n.fn.name !== "loft_solve_jet") throw new Error(`${name}: lib ${n.fn.name}`); return alias(e, n.args[1]!);
      case "fetch": {
        if (E.everything([n.index]).some((c) => ["var", "input", "param"].includes(c.node.op))) throw new Error(`${name}: fetch at a leaf-dependent entry`);
        return alias(e, entry(n.table, Math.trunc(constant(n.index)))[n.lane]!);
      }
      case "iterate": {
        const l = n.loop, count = constant(l.count);
        let state = l.init, alive: Expr<"bool"> = E.lt(E.f(0), E.f(1));
        for (let i = 0; i < count; i++) {
          const next = l.step(state, E.f(i));
          state = l.until === undefined ? next : next.map((v, j) => E.select(alive, v as Expr<"f32">, state[j] as Expr<"f32">));
          if (l.until) alive = E.and(alive, E.not(l.until(next)));
        }
        return alias(e, state[n.k]!);
      }
      default: throw new Error(`${name}: node ${n.op} is outside lane 2.3's bridge`);
    }
    nodes.push(rec); at.set(e.id, nodes.length - 1); return nodes.length - 1;
  };
  return { slots, held, nodes, slot, walk };
}
