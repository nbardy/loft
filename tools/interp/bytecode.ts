// Probe format: a u32 header (opcode, destination), then three u32 operands per instruction.
// Scalars are splatted, so a vector/scalar mix needs no broadcast instruction. Only live
// values occupy the 64 vec4s of per-fragment scratch. Both scenes use the same shader.
import { children, libsOf, topo, variable, type Expr, type LibFn } from "../../src/expr.ts";
import { hash2, noise2 } from "../../src/lib/noise.ts";
import { eyePosition, fwidth } from "../../src/lib/wgsl.ts";

export class UnsupportedPaintOp extends Error {
  override readonly name = "UnsupportedPaintOp";
  constructor(readonly op: string) { super(`unsupported paint instruction: ${op}`); }
}

const x = variable("probe_x", "f32"), p = variable("probe_p", "vec2");
const native = libsOf([fwidth(x), eyePosition(), hash2(p), noise2(p)]);
// These two private sea builtins have no public expression constructors. Pin their bodies
// too: an author-provided lib of the same name cannot silently pick up our implementation.
const extra: LibFn[] = [
  { name: "loft_sea_footprint", ret: "f32", wgsl: "fn loft_sea_footprint(p: vec3<f32>) -> f32 { return sea_footprint(p); }", js: "gpu-only" },
  { name: "loft_fwidth_quad", ret: "f32", wgsl: "@diagnostic(off, derivative_uniformity) fn loft_fwidth_quad(x: f32) -> f32 { return fwidth(x); }", js: "gpu-only" },
];
const libraries = [...native, ...extra];
const canonical = new Map(libraries.map(fn => [fn.name, fn]));
const body = {
  constant: "constants[i.y]", u: "vec4f(uv.x)", v: "vec4f(uv.y)", world: "vec4f(world, 0)",
  input: "vec4f(inputs[i.y])", eye: "vec4f(loft_eye(), 0)",
  x: "a.xxxx", y: "a.yyyy", xz: "vec4f(a.xz, 0, 0)",
  pair: "vec4f(a.x, b.x, 0, 0)", rgba: "vec4f(a.xyz, b.x)",
  add: "a + b", sub: "a - b", mul: "a * b", div: "vec4f(a.x / b.x)",
  abs: "abs(a)", min: "min(a, b)", max: "max(a, b)", floor: "floor(a)",
  mix: "mix(a, b, c)", smoothstep: "vec4f(smoothstep(a.x, b.x, c.x))", clamp: "clamp(a, b, c)",
  sin: "vec4f(sin(a.x))", cos: "vec4f(cos(a.x))", tan: "vec4f(tan(a.x))",
  exp: "vec4f(exp(a.x))", log: "vec4f(log(a.x))", pow: "vec4f(pow(a.x, b.x))",
  atan2: "vec4f(atan2(a.x, b.x))", saturate: "saturate(a)", fract: "fract(a)",
  length2: "vec4f(length(a.xy))", distance3: "vec4f(distance(a.xyz, b.xyz))",
  lt: "vec4f(select(0., 1., a.x < b.x))", select: "select(c, b, a.x != 0.)",
  width: "vec4f(fwidth(a.x))", footprint: "vec4f(loft_sea_footprint(a.xyz))",
  hash2: "vec4f(loft_hash2(a.xy))", noise2: "vec4f(loft_noise2(a.xy))",
} as const;
type Op = keyof typeof body;
const ops = Object.keys(body) as Op[];
const codes = new Map(ops.map((op, code) => [op, code]));
const signature = new Map<string, Op>();
function support(op: Op, fn: string, ...args: string[]) {
  for (const types of args) signature.set(`${fn}(${types})`, op);
}
for (const fn of ["sin", "cos", "tan", "exp", "log", "abs", "fract", "saturate"] as const) support(fn, fn, "f32");
for (const fn of ["min", "max", "atan2", "pow", "mul", "div", "add", "sub", "lt"] as const) support(fn, fn, "f32,f32");
for (const fn of ["smoothstep", "mix", "clamp"] as const) support(fn, fn, "f32,f32,f32");
support("add", "add", "vec2,f32", "vec2,vec2");
support("sub", "sub", "vec2,vec2");
support("mul", "mul", "vec2,f32");
support("floor", "floor", "vec2");
support("mix", "mix", "vec3,vec3,f32");
support("select", "select", "bool,f32,f32", "bool,vec2,vec2");
support("length2", "length", "vec2");
support("distance3", "distance", "vec3,vec3");
support("pair", "vec2", "f32,f32");
support("rgba", "vec4", "vec3,f32");
support("width", "loft_fwidth", "f32");
support("width", "loft_fwidth_quad", "f32");
support("eye", "loft_eye", "");
support("footprint", "loft_sea_footprint", "vec3");
support("hash2", "loft_hash2", "vec2");
support("noise2", "loft_noise2", "vec2");

function lookup(name: string, args: readonly Expr[]) {
  const key = `${name}(${args.map(e => e.ty).join(",")})`;
  const op = signature.get(key);
  if (op === undefined) throw new UnsupportedPaintOp(key);
  return op;
}

function opcode(e: Expr): Op {
  const n = e.node;
  switch (n.op) {
    case "const": return "constant";
    case "input": if (n.ty === "f32") return "input"; throw new UnsupportedPaintOp(`input ${n.spec.name}:${n.ty}`);
    case "var":
      if ((n.name === "u" || n.name === "v") && n.ty === "f32") return n.name;
      if (n.name === "f_world" && n.ty === "vec3") return "world";
      throw new UnsupportedPaintOp(`var ${n.name}:${n.ty}`);
    case "get":
      if (n.sel === "x" || n.sel === "y" || n.sel === "xz") return n.sel;
      throw new UnsupportedPaintOp(`get .${n.sel}`);
    case "lib": {
      const fn = canonical.get(n.fn.name);
      if (!fn || (fn !== n.fn && !(extra.includes(fn) && fn.wgsl === n.fn.wgsl && fn.ret === n.fn.ret && !n.fn.deps?.length)))
        throw new UnsupportedPaintOp(`lib ${n.fn.name}`);
      return lookup(n.fn.name, n.args);
    }
    case "vec": return lookup(n.ty, n.args);
    case "call": return lookup(n.fn, n.args);
    case "param": case "row": case "fetch": case "iterate": throw new UnsupportedPaintOp(n.op);
  }
}

const CAPACITY = 64;
export function lowerPaint(root: Expr, inputSlots: ReadonlyMap<string, number>) {
  if (root.ty !== "vec3" && root.ty !== "vec4") throw new UnsupportedPaintOp(`colour ${root.ty}`);
  const nodes = topo([root]);
  const code = new Uint32Array(4 + nodes.length * 4);
  const pool: number[] = [];
  const uses = new Map<Expr, number>(), regs = new Map<Expr, number>(), free: number[] = [];
  for (const e of nodes) for (const k of children(e)) uses.set(k, (uses.get(k) ?? 0) + 1);
  let peak = 0;
  for (const [pc, e] of nodes.entries()) {
    const op = opcode(e), n = e.node, kids = children(e);
    const at = 4 + pc * 4;
    kids.forEach((k, i) => {
      code[at + 1 + i] = regs.get(k)!;
      const remaining = uses.get(k)! - 1;
      uses.set(k, remaining);
      if (remaining === 0) free.push(regs.get(k)!);
    });
    // All operands are read before the store, so a last-use operand can be the destination.
    const dst = free.pop() ?? peak++;
    if (dst >= CAPACITY) throw new UnsupportedPaintOp(`${op} needs register ${dst}, capacity ${CAPACITY}`);
    regs.set(e, dst);
    code[at] = codes.get(op)! | (dst << 8);
    if (n.op === "const") {
      code[at + 1] = pool.length / 4;
      for (let c = 0; c < 4; c++) pool.push(n.v.length === 1 ? n.v[0] : (n.v[c] ?? 0));
    }
    if (n.op === "input") {
      const slot = inputSlots.get(n.spec.name);
      if (slot === undefined) throw new UnsupportedPaintOp(`unbound input ${n.spec.name}`);
      code[at + 1] = slot;
    }
  }
  code.set([nodes.length, regs.get(root)!, Number(root.ty === "vec3"), 0]);
  return { code, constants: new Float32Array(pool), peak, count: nodes.length };
}

export const interpreter = `
${libraries.map(fn => fn.wgsl).join("\n")}
struct Program { header: vec4u, code: array<vec4u> }
@group(1) @binding(0) var<storage, read> prog: Program;
@group(1) @binding(1) var<storage, read> constants: array<vec4f>;
@fragment fn fs(@builtin(position) pixel: vec4f) -> @location(0) vec4f {
  let uv = pixel.xy / 512.;
  let world = world_at(uv);
  var r: array<vec4f, ${CAPACITY}>;
  // PC and opcode come only from read-only storage and advance identically across each quad.
  for (var pc = 0u; pc < prog.header.x; pc++) {
    let i = prog.code[pc];
    let a = r[i.y & ${CAPACITY - 1}u]; let b = r[i.z & ${CAPACITY - 1}u]; let c = r[i.w & ${CAPACITY - 1}u];
    var value: vec4f;
    switch (i.x & 255u) {
      ${ops.map((op, k) => `case ${k}u: { value = ${body[op]}; }`).join("\n      ")}
      default: { return vec4f(-65504., 65504., -65504., 65504.); }
    }
    r[(i.x >> 8u) & ${CAPACITY - 1}u] = value;
  }
  let colour = r[prog.header.y & ${CAPACITY - 1}u];
  return select(colour, vec4f(colour.xyz, 1.), prog.header.z != 0u);
}`;
