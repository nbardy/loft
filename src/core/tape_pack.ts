// Expressions as data for the Rust core's tape (rust/loft-core/src/tape.rs), the twin of `compileJs`: the nodes the
// roots reach, dependencies first, each a record of numbers. Leaves are resolved here, once: a variable to its slot
// in the vars the kernel sets, `time` to the clock, an input or a param to the value `compileJs` would read for it.
// A node the tape cannot evaluate (a library function, whose CPU twin is JavaScript; a per-frame table; a loop) is a
// refusal, named, and the caller keeps the TypeScript path.

import { topo, type Expr, type Fn, type InputSpec, type ParamSpec, type Row } from "../expr.ts";
import type { Writer } from "./wire.ts";

/** Node kinds (tape.rs OP_*). */
const OP = { const: 0, var: 1, leaf: 2, time: 3, row: 4, call: 5, vec: 6, get: 7 } as const;

/** Call functions by number: the order of tape.rs `Fn`. */
const FN: readonly Fn[] = [
  "add", "sub", "mul", "div", "neg", "min", "max", "pow", "mod", "atan2", "step",
  "abs", "sign", "floor", "fract", "sqrt", "exp", "log", "sin", "cos", "tan", "saturate", "normalize",
  "mix", "clamp", "smoothstep", "length", "distance", "dot", "cross",
  "lt", "gt", "le", "ge", "eq", "ne", "and", "or", "not", "select",
  "ceil", "trunc", "roundEven", "rem", "inverseSqrt", "exp2", "log2", "asin", "acos", "atan", "sinh", "cosh", "tanh",
];
const FN_CODE = new Map<Fn, number>(FN.map((f, i) => [f, i]));
const WIDTH = { f32: 1, vec2: 2, vec3: 3, vec4: 4, bool: 1 } as const;

/** How the leaves read, as `compileJs` reads them from an `Env`. */
export type Leaves = {
  /** A variable's slot in the vars the kernel sets, or undefined (refused: the tape reads only those). */
  readonly variable: (name: string) => number | undefined;
  /** The input's components (`inputs[name] ?? def`), or "time" for the clock the kernel sets. */
  readonly input: (spec: InputSpec) => readonly number[] | "time";
  /** `params[name] ?? def`. */
  readonly param: (spec: ParamSpec) => number;
};

/** Writes the program for `roots`, or returns why the tape cannot run it (nothing usable is written then). */
export function packTape(w: Writer, roots: readonly Expr[], leaves: Leaves): string | undefined {
  const list = topo(roots);
  const index = new Map<number, number>();
  const rows: Row[] = [];
  // Refuse before writing anything.
  for (const e of list) {
    const n = e.node;
    switch (n.op) {
      case "lib": return `library function "${n.fn.name}" (its CPU twin is JavaScript)`;
      case "fetch": return `per-frame table "${n.table.name}"`;
      case "iterate": return "a loop (iterate)";
      case "var": if (leaves.variable(n.name) === undefined || n.ty !== "f32") return `variable "${n.name}" (${n.ty}) the kernel does not set`; break;
      case "call": if (!FN_CODE.has(n.fn)) return `function "${n.fn}"`; break;
      case "row": if (!rows.includes(n.row)) rows.push(n.row); break;
      default: break;
    }
  }
  w.u32(rows.length);
  for (const r of rows) w.f64s(r.samples);
  w.u32(list.length);
  list.forEach((e, i) => {
    index.set(e.id, i);
    const n = e.node;
    const kid = (x: Expr): number => index.get(x.id)!;
    const head = (op: number) => { w.u32(op); w.u32(WIDTH[n.ty]); w.u32(n.ty === "bool" ? 1 : 0); };
    switch (n.op) {
      case "const": head(OP.const); w.f64s(n.v); break;
      case "var": head(OP.var); w.u32(leaves.variable(n.name)!); break;
      case "input": {
        const v = leaves.input(n.spec);
        if (v === "time") head(OP.time); else { head(OP.leaf); w.f64s(v); }
        break;
      }
      case "param": head(OP.leaf); w.f64s([leaves.param(n.spec)]); break;
      case "row": head(OP.row); w.u32(rows.indexOf(n.row)); w.u32(kid(n.x)); break;
      case "call": head(OP.call); w.u32(FN_CODE.get(n.fn)!); w.u32(n.args.length); for (const a of n.args) w.u32(kid(a)); break;
      case "vec": head(OP.vec); w.u32(n.args.length); for (const a of n.args) w.u32(kid(a)); break;
      case "get": head(OP.get); w.u32(kid(n.a)); w.u32(n.sel.length); for (const ch of n.sel) w.u32("xyzw".indexOf(ch)); break;
      case "lib": case "fetch": case "iterate": throw new Error("internal: refused above");
    }
  });
  w.u32(roots.length);
  for (const r of roots) w.u32(index.get(r.id)!);
  return undefined;
}
