// Constant folding for expressions the CPU evaluates every frame.
//
// WORKAROUND for a gap in src/expr.ts: `compileJs` evaluates every node on every call, including subgraphs whose
// leaves are all constants (a tip law at a station's fixed m, a wave at a control point's fixed fraction). The pose
// program has hundreds of those per frame. This pass evaluates them once at compile time and rebuilds the graph with
// the public constructors. Curve-row lookups are never folded (their tables can be edited at run time).
// The proper fix is folding inside `compileJs` (or in `Expr.make`).

import * as X from "../expr.ts";
import { lift, R, type Expr } from "../expr.ts";

type Builder = (...args: never[]) => Expr;
const CALLS: Readonly<Record<string, Builder>> = {
  add: X.add, sub: X.sub, mul: X.mul, div: X.div, min: X.min, max: X.max, pow: X.pow, mod: X.mod, atan2: X.atan2,
  step: X.step, neg: X.neg, abs: X.abs, sign: X.sign, floor: X.floor, fract: X.fract, sqrt: X.sqrt, exp: X.exp,
  log: X.log, sin: X.sin, cos: X.cos, tan: X.tan, saturate: X.saturate, normalize: X.normalize, mix: X.mix,
  clamp: X.clamp, smoothstep: X.smoothstep, length: X.length, distance: X.distance, dot: X.dot, cross: X.cross,
  lt: X.lt, gt: X.gt, le: X.le, ge: X.ge, and: X.and, or: X.or, not: X.not, select: X.select,
} as Record<string, Builder>;
const VECS = { vec2: X.vec2, vec3: X.vec3, vec4: X.vec4 } as const;
const GETS: Readonly<Record<string, (e: Expr) => Expr>> = {
  x: (e) => e.x, y: (e) => e.y, z: (e) => e.z, w: (e) => e.w, xy: (e) => e.xy, xz: (e) => e.xz, xyz: (e) => e.xyz,
};

type Value = number | number[] | boolean;
/** The value of a constant node, as the JavaScript backend represents it. */
function valueOf(e: Expr): Value {
  const n = e.node;
  if (n.op !== "const") throw new Error("valueOf: not a constant");
  return n.ty === "bool" ? n.v[0] !== 0 : n.v.length === 1 ? n.v[0] : [...n.v];
}
const SIZE = { vec2: 2, vec3: 3, vec4: 4 } as const;
const RT = R as unknown as Record<string, (...a: unknown[]) => Value>;

export function foldConstants(roots: readonly Expr[]): Expr[] {
  const done = new Map<number, Expr>();
  const isConst = (e: Expr) => e.node.op === "const";
  // Values are computed by the same runtime `compileJs` calls, so folding cannot change a result.
  const constant = (v: Value): Expr => lift(v as never);
  const f = (c: Expr) => done.get(c.id)!;
  const fold = (e: Expr): Expr => {
    const n = e.node;
    switch (n.op) {
      case "const": case "var": case "input": case "param": return e;
      case "row": return X.row(n.row, f(n.x) as Expr<"f32">);
      // Library functions may be GPU-only (no CPU twin), so they are rebuilt, never folded.
      case "lib": return X.lib(n.fn, ...n.args.map(f));
      case "vec": {
        const args = n.args.map(f);
        return args.every(isConst) ? constant(R.vec(SIZE[n.ty], ...(args.map(valueOf) as number[]))) : VECS[n.ty](...(args as X.Num[]));
      }
      case "get": {
        const g = GETS[n.sel];
        if (!g) throw new Error(`foldConstants: no public constructor for selector .${n.sel}`);
        const a = f(n.a);
        return isConst(a) ? constant(R.get(valueOf(a) as number[], n.sel)) : g(a);
      }
      case "call": {
        const build = CALLS[n.fn];
        if (!build) throw new Error(`foldConstants: no public constructor for ${n.fn}`);
        const args = n.args.map(f);
        return args.every(isConst) ? constant(RT[n.fn](...args.map(valueOf))) : (build as (...a: Expr[]) => Expr)(...args);
      }
      default: throw new Error(`foldConstants: unknown node ${JSON.stringify((n as { op: string }).op)}`);
    }
  };
  for (const e of X.topo(roots)) done.set(e.id, fold(e));
  return roots.map((r) => done.get(r.id)!);
}
