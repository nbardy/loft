// Lane 1.3 check (a) input: every `tests/expr*` fixture and one case per operator, as graphs plus `compileJs` (f64)
// values at f32 points. Until the recorder (lane 1.2) lands, this walk of the TS graph is the bridge.
// Usage: bun rust/crates/loft-core/dump_math.ts > out/lr13/math.json
import {
  abs, add, atan2, clamp, compileJs, cos, cross, eq, f, floor, length, lt, mix, mod, mul, ne, normalize, param, pick, pow,
  round, select, sin, smoothstep, step, sub, time, variable, vec3, type Expr, type Fn,
} from "../../../src/expr.ts";
import * as E from "../../../src/expr.ts";

const FNS: Fn[] = [
  "add", "sub", "mul", "div", "neg", "min", "max", "pow", "mod", "atan2", "step",
  "abs", "sign", "floor", "fract", "sqrt", "exp", "log", "sin", "cos", "tan", "saturate", "normalize",
  "mix", "clamp", "smoothstep", "length", "distance", "dot", "cross",
  "lt", "gt", "le", "ge", "eq", "ne", "and", "or", "not", "select",
  "ceil", "trunc", "roundEven", "rem", "inverseSqrt", "exp2", "log2", "asin", "acos", "atan", "sinh", "cosh", "tanh",
];
const SEL = "xyzw";
type Case = { name: string; root: Expr; dom: Record<string, [number, number]> };
const u = variable("u", "f32"), x = variable("x", "f32");
const a = variable("a", "f32"), b = variable("b", "f32"), c = variable("c", "f32");
const girth = param("test.girth", 1.3);

// tests/expr.test.ts "JS and WGSL backends agree" (the `row` case is excluded: curve rows are lane 2.1 resources).
const unit: Record<string, [number, number]> = { u: [0, 1], time: [0, 3], "test.girth": [0.5, 2] };
const cases: Case[] = [
  add(mul(sin(mul(u, 7)), girth), cos(time)),
  mix(vec3(1, 2, 3), vec3(u, mul(u, u), 0.5), smoothstep(0.2, 0.8, u)),
  normalize(cross(vec3(u, 1, 0.3), vec3(0.2, u, 1))).mul(length(vec3(u, 2, 3))),
  mod(sub(mul(u, 10), 3.7), 1.3),
  select(lt(u, 0.5), pow(u, 2.2), atan2(u, 0.3)),
  clamp(vec3(u, -1, 4), 0, vec3(0.6, 0.6, 2)),
  mix(u, vec3(1, 2, 3), 0.3),
  select(lt(u, 0.4), u, vec3(1, 2, 3)),
  round(mul(vec3(u, 2.5, -1.5), 3)),
  mul(step(0.25, u), u),
].map((root, i) => ({ name: `expr.test#${i}`, root, dom: unit }));
// tests/expr.eq.gpu.test.ts
{
  const k = floor(x), index = clamp(add(k, 5), 0, 9);
  [select(eq(k, 2), 1, 0), select(ne(k, -3), 1, 0), select(eq(mul(x, 0), f(0)), 1, 0), pick(index, [10, 11, 12, 13, 14, 15, 16, 17, 18, 19])]
    .forEach((root, i) => cases.push({ name: `expr.eq#${i}`, root, dom: { x: [-10, 10] } }));
}
// One case per operator, on a domain where it is finite.
const call = (E as unknown as Record<string, (...a: unknown[]) => Expr>);
const pos: [number, number] = [0.05, 4], any: [number, number] = [-4, 4], one: [number, number] = [-0.99, 0.99];
const domains: Partial<Record<Fn, Record<string, [number, number]>>> = {
  pow: { a: pos, b: [-3, 3] }, sqrt: { a: [0, 9] }, log: { a: pos }, log2: { a: pos }, inverseSqrt: { a: pos },
  asin: { a: one }, acos: { a: one }, div: { a: any, b: [0.25, 4] }, mod: { a: any, b: [0.25, 4] }, rem: { a: any, b: [0.25, 4] },
  tan: { a: [-1.4, 1.4] }, exp: { a: [-20, 20] }, exp2: { a: [-20, 20] }, sinh: { a: [-9, 9] }, cosh: { a: [-9, 9] },
  sin: { a: [-100, 100] }, cos: { a: [-100, 100] },
};
for (const fn of FNS) {
  const v = vec3(a, b, c), w = vec3(b, c, a);
  const name = fn === "roundEven" ? "roundEven" : fn;
  const root: Expr =
    ["normalize", "length"].includes(fn) ? call[name]!(v)
    : ["distance", "dot", "cross"].includes(fn) ? call[name]!(v, w)
    : ["and", "or"].includes(fn) ? call[name]!(lt(a, b), gt(b, c))
    : fn === "not" ? call[name]!(lt(a, b))
    : fn === "select" ? select(lt(a, 0), b, c)
    : ["neg", "abs", "sign", "floor", "fract", "sqrt", "exp", "log", "sin", "cos", "tan", "saturate", "ceil", "trunc", "roundEven", "inverseSqrt", "exp2", "log2", "asin", "acos", "atan", "sinh", "cosh", "tanh"].includes(fn) ? call[name]!(a)
    : ["mix", "clamp", "smoothstep"].includes(fn) ? (fn === "clamp" ? clamp(a, b, add(b, abs(c))) : fn === "smoothstep" ? smoothstep(b, add(b, add(abs(c), 0.5)), a) : mix(a, b, c))
    : call[name]!(a, b);
  cases.push({ name: `op:${fn}`, root, dom: { a: any, b: any, c: any, ...domains[fn] } });
}
function gt(p: Expr, q: Expr): Expr { return lt(q, p); }

// Mulberry32: a fixed corpus.
let seed = 0x13a7;
const rnd = (): number => { seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const out = cases.map(({ name, root, dom }) => {
  const slots: string[] = [], nodes: unknown[] = [], at = new Map<number, number>();
  const slot = (s: string): number => (slots.includes(s) ? slots.indexOf(s) : slots.push(s) - 1);
  const walk = (e: Expr): number => {
    const seen = at.get(e.id); if (seen !== undefined) return seen;
    const n = e.node;
    const rec =
      n.op === "const" ? { k: "lit", v: n.v.map(Number) }
      : n.op === "var" ? { k: "leaf", slot: slot(n.name) }
      : n.op === "input" || n.op === "param" ? { k: "leaf", slot: slot(n.spec.name) }
      : n.op === "call" ? { k: "call", fn: FNS.indexOf(n.fn), args: n.args.map(walk) }
      : n.op === "vec" ? { k: "vec", w: { vec2: 2, vec3: 3, vec4: 4 }[n.ty], args: n.args.map(walk) }
      : n.op === "get" ? { k: "get", a: walk(n.a), sel: [...n.sel].map((ch) => SEL.indexOf(ch)) }
      : (() => { throw new Error(`${name}: node ${n.op} is outside lane 1.3`); })();
    nodes.push(rec); at.set(e.id, nodes.length - 1); return nodes.length - 1;
  };
  const root_ = walk(root);
  const run = compileJs([root]);
  const points: number[][] = [], want: number[][] = [];
  for (let i = 0; i < 64; i++) {
    const p = slots.map((s) => { const [lo, hi] = dom[s]!; return Math.fround(lo + (hi - lo) * rnd()); });
    const env = { vars: {} as Record<string, number>, inputs: {} as Record<string, number>, params: {} as Record<string, number> };
    slots.forEach((s, k) => { env.vars[s] = p[k]!; env.inputs[s] = p[k]!; env.params[s] = p[k]!; });
    const r = run(env)[0]!;
    points.push(p); want.push((Array.isArray(r) ? r : [r]).map(Number));
  }
  return { name, slots, nodes, root: root_, points, want };
});
console.log(JSON.stringify({ cases: out }));
