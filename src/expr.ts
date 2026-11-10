// Loft L0: symbolic maths.
//
// An `Expr` is a node in a hash-consed DAG. Authoring code never computes numbers; it builds expressions, and two
// backends turn the same DAG into code: JavaScript (CPU: the pose pass, checks, baking) and WGSL (GPU: vertex and
// fragment stages). Everything above this layer (curves, charts, shapes, styles) is written in terms of `Expr`, which
// is what lets one description be evaluated wherever the schedule says.
//
// Rules: expressions are pure. A leaf is a constant, a free variable (bound by the consumer: u, v, a world point), a
// host input (time, game state), a tunable parameter (a named number that can change without recompiling), or a
// lookup into a table-backed curve.

export type Ty = "f32" | "vec2" | "vec3" | "vec4" | "bool";
export type NumTy = Exclude<Ty, "bool">;
export type VecTy = Exclude<NumTy, "f32">;

const SIZE: Record<NumTy, number> = { f32: 1, vec2: 2, vec3: 3, vec4: 4 };
const TY_OF_SIZE: readonly NumTy[] = ["f32", "f32", "vec2", "vec3", "vec4"];

/** Samples per table-backed curve row. Each sample is a vec4. */
export const CURVE_N = 64;

/** A table-backed curve: CURVE_N vec4 samples over x in [0, 1], linearly interpolated, clamped at the ends. */
export class Row {
  private static next = 0;
  readonly id: number;
  constructor(readonly samples: Float32Array, readonly label: string) {
    if (samples.length !== CURVE_N * 4) {
      throw new Error(`Row "${label}": expected ${CURVE_N * 4} floats, got ${samples.length}`);
    }
    this.id = Row.next++;
  }
}

export type ParamSpec = { readonly name: string; readonly def: number; readonly min: number; readonly max: number };
export type InputSpec = { readonly name: string; readonly ty: NumTy; readonly def: readonly number[] };

type Fn =
  | "add" | "sub" | "mul" | "div" | "neg" | "min" | "max" | "pow" | "mod" | "atan2" | "step"
  | "abs" | "sign" | "floor" | "fract" | "sqrt" | "exp" | "log" | "sin" | "cos" | "tan" | "saturate" | "normalize"
  | "mix" | "clamp" | "smoothstep" | "length" | "distance" | "dot" | "cross"
  | "lt" | "gt" | "le" | "ge" | "and" | "or" | "not" | "select";

/**
 * A named function written once in each backend's language: the way to add vocabulary (noise, a custom falloff) that
 * the built-in operators cannot express compactly. `wgsl` is the full function definition, named `name`. `js` is the
 * CPU twin, or "gpu-only" for functions that may only be used in shader stages (evaluating one on the CPU throws).
 * `deps` are other library functions the WGSL body calls.
 */
export type LibFn = {
  readonly name: string;
  readonly ret: Ty;
  readonly wgsl: string;
  readonly js: ((...args: any[]) => number | number[] | boolean) | "gpu-only";
  readonly deps?: readonly LibFn[];
};

type Node =
  | { readonly op: "lib"; readonly ty: Ty; readonly fn: LibFn; readonly args: readonly Expr[] }
  | { readonly op: "const"; readonly ty: Ty; readonly v: readonly number[] }
  | { readonly op: "var"; readonly ty: Ty; readonly name: string }
  | { readonly op: "input"; readonly ty: NumTy; readonly spec: InputSpec }
  | { readonly op: "param"; readonly ty: "f32"; readonly spec: ParamSpec }
  | { readonly op: "row"; readonly ty: "vec4"; readonly row: Row; readonly x: Expr<"f32"> }
  | { readonly op: "call"; readonly ty: Ty; readonly fn: Fn; readonly args: readonly Expr[] }
  | { readonly op: "vec"; readonly ty: VecTy; readonly args: readonly Expr[] }
  | { readonly op: "get"; readonly ty: NumTy; readonly a: Expr; readonly sel: string };

type Wider<A extends NumTy, B extends NumTy> = A extends "f32" ? B : A;
/** A vector written as a plain array. TypeScript cannot see its length, so its result type is "some vector". */
export type Lit = readonly number[];
/** Anything accepted where an expression of type T is wanted: plain numbers lift to constants. */
export type In<T extends Ty = Ty> = Expr<T> | (T extends "f32" ? number : T extends "bool" ? boolean : readonly number[]);
export type Num = Expr<NumTy> | number | readonly number[];

const interned = new Map<string, Expr>();
const params = new Map<string, ParamSpec>();
const inputs = new Map<string, InputSpec>();

export class Expr<T extends Ty = Ty> {
  private constructor(readonly id: number, readonly node: Node) {}
  get ty(): T { return this.node.ty as T; }

  static make<T extends Ty>(node: Node, key: string): Expr<T> {
    const hit = interned.get(key);
    if (hit) return hit as Expr<T>;
    const made = new Expr<T>(interned.size, node);
    interned.set(key, made);
    return made;
  }

  // Fluent forms of the common operators. `a.add(b)` is `add(a, b)`.
  add(b: number): Expr<T>;
  add<B extends NumTy>(b: Expr<B>): Expr<Wider<T & NumTy, B>>;
  add(b: Lit): Expr<T extends "f32" ? VecTy : T>;
  add<B extends NumTy = "f32">(b: Expr<B> | number | Lit): Expr<Wider<T & NumTy, B>>;
  add(b: In<NumTy>): Expr { return add(this as never, b as never); }
  sub(b: number): Expr<T>;
  sub<B extends NumTy>(b: Expr<B>): Expr<Wider<T & NumTy, B>>;
  sub(b: Lit): Expr<T extends "f32" ? VecTy : T>;
  sub<B extends NumTy = "f32">(b: Expr<B> | number | Lit): Expr<Wider<T & NumTy, B>>;
  sub(b: In<NumTy>): Expr { return sub(this as never, b as never); }
  mul(b: number): Expr<T>;
  mul<B extends NumTy>(b: Expr<B>): Expr<Wider<T & NumTy, B>>;
  mul(b: Lit): Expr<T extends "f32" ? VecTy : T>;
  mul<B extends NumTy = "f32">(b: Expr<B> | number | Lit): Expr<Wider<T & NumTy, B>>;
  mul(b: In<NumTy>): Expr { return mul(this as never, b as never); }
  div(b: number): Expr<T>;
  div<B extends NumTy>(b: Expr<B>): Expr<Wider<T & NumTy, B>>;
  div(b: Lit): Expr<T extends "f32" ? VecTy : T>;
  div<B extends NumTy = "f32">(b: Expr<B> | number | Lit): Expr<Wider<T & NumTy, B>>;
  div(b: In<NumTy>): Expr { return div(this as never, b as never); }
  get x(): Expr<"f32"> { return get(this, "x"); }
  get y(): Expr<"f32"> { return get(this, "y"); }
  get z(): Expr<"f32"> { return get(this, "z"); }
  get w(): Expr<"f32"> { return get(this, "w"); }
  get xy(): Expr<"vec2"> { return get(this, "xy"); }
  get xz(): Expr<"vec2"> { return get(this, "xz"); }
  get xyz(): Expr<"vec3"> { return get(this, "xyz"); }
}

// ---- leaves -------------------------------------------------------------------------------------------------------

function constant(ty: Ty, v: readonly number[]): Expr {
  for (const n of v) if (!Number.isFinite(n)) throw new Error(`non-finite constant ${n}`);
  return Expr.make({ op: "const", ty, v }, `c:${ty}:${v.join(",")}`);
}

/** Lift a plain value to an expression. Numbers become f32, arrays become vectors, booleans become bool. */
export function lift<T extends Ty>(x: In<T>): Expr<T> {
  if (x instanceof Expr) return x;
  if (typeof x === "number") return constant("f32", [x]) as Expr<T>;
  if (typeof x === "boolean") return constant("bool", [x ? 1 : 0]) as Expr<T>;
  const arr = x as readonly number[];
  if (arr.length < 2 || arr.length > 4) throw new Error(`cannot lift an array of length ${arr.length}`);
  return constant(TY_OF_SIZE[arr.length], arr) as Expr<T>;
}

export const f = (x: number): Expr<"f32"> => lift<"f32">(x);

/** A free variable, bound by whoever consumes the expression (u, v, rho, a world point). */
export function variable<T extends Ty>(name: string, ty: T): Expr<T> {
  return Expr.make({ op: "var", ty, name }, `v:${ty}:${name}`);
}

/** A value the host supplies every frame (time, game state, a target position). */
export function input<T extends NumTy>(name: string, ty: T, def: readonly number[]): Expr<T> {
  if (def.length !== SIZE[ty]) throw new Error(`input "${name}": default has ${def.length} components, ${ty} needs ${SIZE[ty]}`);
  const spec: InputSpec = { name, ty, def };
  const seen = inputs.get(name);
  if (seen && (seen.ty !== ty || seen.def.join() !== def.join())) throw new Error(`input "${name}" declared twice with different types or defaults`);
  inputs.set(name, spec);
  return Expr.make({ op: "input", ty, spec }, `i:${name}`);
}

/** Seconds since the scene started. */
export const time: Expr<"f32"> = input("time", "f32", [0]);

/** A named tunable number. Changing it at run time is a buffer write, never a recompile. */
export function param(name: string, def: number, range?: { min?: number; max?: number }): Expr<"f32"> {
  const span = Math.max(Math.abs(def), 1e-3);
  const spec: ParamSpec = { name, def, min: range?.min ?? def - 2 * span, max: range?.max ?? def + 2 * span };
  const seen = params.get(name);
  if (seen && seen.def !== def) throw new Error(`param "${name}" declared twice with different defaults (${seen.def}, ${def})`);
  params.set(name, seen ?? spec);
  return Expr.make({ op: "param", ty: "f32", spec: seen ?? spec }, `p:${name}`);
}

/** Look up a table-backed curve at x in [0, 1]. Always a vec4; take components with `.x`, `.xyz`. */
export function row(r: Row, x: In<"f32">): Expr<"vec4"> {
  const xe = lift<"f32">(x);
  return Expr.make({ op: "row", ty: "vec4", row: r, x: xe }, `r:${r.id}:${xe.id}`);
}

const libs = new Map<string, LibFn>();
/** Call a library function. Two different functions may not share a name. */
export function lib<T extends Ty>(fn: LibFn, ...args: In[]): Expr<T> {
  const seen = libs.get(fn.name);
  if (seen && seen !== fn) throw new Error(`library function "${fn.name}" is defined twice`);
  libs.set(fn.name, fn);
  const xs = args.map((a) => lift(a));
  return Expr.make({ op: "lib", ty: fn.ret, fn, args: xs }, `l:${fn.name}:${xs.map((a) => a.id).join(",")}`);
}

// ---- constructors and access --------------------------------------------------------------------------------------

function vecOf(ty: VecTy, parts: readonly Num[]): Expr {
  const args = parts.map((p) => lift(p as In));
  const total = args.reduce((n, a) => n + SIZE[a.ty as NumTy], 0);
  if (total !== SIZE[ty] && !(args.length === 1 && args[0].ty === "f32")) {
    throw new Error(`${ty}(...) got ${total} components`);
  }
  return Expr.make({ op: "vec", ty, args }, `V:${ty}:${args.map((a) => a.id).join(",")}`);
}
export const vec2 = (...p: Num[]): Expr<"vec2"> => vecOf("vec2", p) as Expr<"vec2">;
export const vec3 = (...p: Num[]): Expr<"vec3"> => vecOf("vec3", p) as Expr<"vec3">;
export const vec4 = (...p: Num[]): Expr<"vec4"> => vecOf("vec4", p) as Expr<"vec4">;

function get<T extends NumTy>(a: Expr, sel: string): Expr<T> {
  if (a.ty === "bool" || a.ty === "f32") throw new Error(`cannot take .${sel} of ${a.ty}`);
  const size = SIZE[a.ty as NumTy];
  for (const ch of sel) if ("xyzw".indexOf(ch) < 0 || "xyzw".indexOf(ch) >= size) throw new Error(`.${sel} is out of range for ${a.ty}`);
  return Expr.make({ op: "get", ty: TY_OF_SIZE[sel.length], a, sel }, `g:${a.id}:${sel}`);
}

// ---- operators ----------------------------------------------------------------------------------------------------

function wider(...tys: Ty[]): NumTy {
  let out: NumTy = "f32";
  for (const t of tys) {
    if (t === "bool") throw new Error("a bool is not a number here");
    if (t === "f32") continue;
    if (out !== "f32" && out !== t) throw new Error(`mismatched vector types ${out} and ${t}`);
    out = t;
  }
  return out;
}

function call(fn: Fn, ty: Ty, xs: readonly In[]): Expr {
  const args = xs.map((x) => lift(x));
  return Expr.make({ op: "call", ty, fn, args }, `f:${fn}:${args.map((a) => a.id).join(",")}`);
}

// Overloads, not one generic signature: a type parameter cannot be inferred from a plain `number` argument, and with
// a single signature `mul(x, 2)` came out as "some numeric type" instead of the type of `x` (found by the first code
// written against this file).
interface Binary {
  (a: number, b: number): Expr<"f32">;
  <A extends NumTy>(a: Expr<A>, b: number): Expr<A>;
  <B extends NumTy>(a: number, b: Expr<B>): Expr<B>;
  <A extends NumTy, B extends NumTy>(a: Expr<A>, b: Expr<B>): Expr<Wider<A, B>>;
  <A extends NumTy>(a: Expr<A>, b: Lit): Expr<A extends "f32" ? VecTy : A>;
  <B extends NumTy>(a: Lit, b: Expr<B>): Expr<B extends "f32" ? VecTy : B>;
  (a: Lit, b: Lit | number): Expr<VecTy>;
  (a: number, b: Lit): Expr<VecTy>;
  // Last: arguments typed as "an expression or a plain value" (a function parameter of type `In<"f32">`). The type
  // parameter is inferred from the expression half of the union; where the argument is only a number there is nothing
  // to infer from, so it defaults to "f32" (what a number lifts to) instead of the whole constraint.
  <A extends NumTy = "f32", B extends NumTy = "f32">(a: Expr<A> | number | Lit, b: Expr<B> | number | Lit): Expr<Wider<A, B>>;
}
interface Unary {
  (a: number): Expr<"f32">;
  <A extends NumTy>(a: Expr<A>): Expr<A>;
  (a: Lit): Expr<VecTy>;
  <A extends NumTy = "f32">(a: Expr<A> | number | Lit): Expr<A>;
}
const binary = (fn: Fn): Binary => ((a: In<NumTy>, b: In<NumTy>) => call(fn, wider(lift(a).ty, lift(b).ty), [a, b])) as Binary;
const unary = (fn: Fn): Unary => ((a: In<NumTy>) => call(fn, lift(a).ty, [a])) as Unary;

export const add = binary("add"), sub = binary("sub"), mul = binary("mul"), div = binary("div");
export const min = binary("min"), max = binary("max"), pow = binary("pow");
/** Floored modulo (the result has the sign of the divisor), unlike WGSL's `%`. */
export const mod = binary("mod");
/** atan2(y, x). */
export const atan2 = binary("atan2");
/** step(edge, x): 0 below the edge, 1 at or above it. */
export const step = binary("step");
export const neg = unary("neg"), abs = unary("abs"), sign = unary("sign"), floor = unary("floor"), fract = unary("fract");
export const sqrt = unary("sqrt"), exp = unary("exp"), log = unary("log");
export const sin = unary("sin"), cos = unary("cos"), tan = unary("tan");
/** Clamp to [0, 1]. */
export const saturate = unary("saturate");
export const normalize = unary("normalize");

export function mix(a: number, b: number, t: In<NumTy>): Expr<"f32">;
export function mix<A extends NumTy, B extends NumTy>(a: Expr<A>, b: Expr<B>, t: In<NumTy>): Expr<Wider<A, B>>;
export function mix<A extends NumTy>(a: Expr<A>, b: number, t: In<NumTy>): Expr<A>;
export function mix<B extends NumTy>(a: number, b: Expr<B>, t: In<NumTy>): Expr<B>;
export function mix<A extends NumTy>(a: Expr<A>, b: Lit, t: In<NumTy>): Expr<A extends "f32" ? VecTy : A>;
export function mix<B extends NumTy>(a: Lit, b: Expr<B>, t: In<NumTy>): Expr<B extends "f32" ? VecTy : B>;
export function mix(a: Lit, b: Lit, t: In<NumTy>): Expr<VecTy>;
export function mix<A extends NumTy = "f32", B extends NumTy = "f32">(a: Expr<A> | number | Lit, b: Expr<B> | number | Lit, t: In<NumTy>): Expr<Wider<A, B>>;
export function mix(a: In<NumTy>, b: In<NumTy>, t: In<NumTy>): Expr {
  return call("mix", wider(lift(a).ty, lift(b).ty), [a, b, t]);
}
export function clamp(x: number, lo: number, hi: number): Expr<"f32">;
export function clamp<A extends NumTy>(x: Expr<A>, lo: In<NumTy>, hi: In<NumTy>): Expr<A>;
export function clamp(x: Lit, lo: In<NumTy>, hi: In<NumTy>): Expr<VecTy>;
export function clamp<A extends NumTy = "f32">(x: Expr<A> | number | Lit, lo: In<NumTy>, hi: In<NumTy>): Expr<A>;
export function clamp(x: In<NumTy>, lo: In<NumTy>, hi: In<NumTy>): Expr {
  return call("clamp", lift(x).ty, [x, lo, hi]);
}
export function smoothstep(lo: number, hi: number, x: number): Expr<"f32">;
export function smoothstep<A extends NumTy>(lo: In<NumTy>, hi: In<NumTy>, x: Expr<A>): Expr<A>;
export function smoothstep<A extends NumTy = "f32">(lo: In<NumTy>, hi: In<NumTy>, x: Expr<A> | number | Lit): Expr<A>;
export function smoothstep(lo: In<NumTy>, hi: In<NumTy>, x: In<NumTy>): Expr {
  return call("smoothstep", wider(lift(lo).ty, lift(hi).ty, lift(x).ty), [lo, hi, x]);
}
export const length = (a: In<NumTy>): Expr<"f32"> => call("length", "f32", [a]) as Expr<"f32">;
export const distance = (a: In<NumTy>, b: In<NumTy>): Expr<"f32"> => call("distance", "f32", [a, b]) as Expr<"f32">;
export const dot = (a: In<NumTy>, b: In<NumTy>): Expr<"f32"> => call("dot", "f32", [a, b]) as Expr<"f32">;
export const cross = (a: In<"vec3">, b: In<"vec3">): Expr<"vec3"> => call("cross", "vec3", [a, b]) as Expr<"vec3">;

const compare = (fn: Fn) => (a: In<"f32">, b: In<"f32">): Expr<"bool"> => call(fn, "bool", [a, b]) as Expr<"bool">;
export const lt = compare("lt"), gt = compare("gt"), le = compare("le"), ge = compare("ge");
export const and = (a: In<"bool">, b: In<"bool">): Expr<"bool"> => call("and", "bool", [a, b]) as Expr<"bool">;
export const or = (a: In<"bool">, b: In<"bool">): Expr<"bool"> => call("or", "bool", [a, b]) as Expr<"bool">;
export const not = (a: In<"bool">): Expr<"bool"> => call("not", "bool", [a]) as Expr<"bool">;
/** `yes` where the condition holds, otherwise `no`. Both sides are always evaluated. */
export function select(cond: In<"bool">, yes: number, no: number): Expr<"f32">;
export function select<A extends NumTy, B extends NumTy>(cond: In<"bool">, yes: Expr<A>, no: Expr<B>): Expr<Wider<A, B>>;
export function select<A extends NumTy>(cond: In<"bool">, yes: Expr<A>, no: number): Expr<A>;
export function select<B extends NumTy>(cond: In<"bool">, yes: number, no: Expr<B>): Expr<B>;
export function select<A extends NumTy>(cond: In<"bool">, yes: Expr<A>, no: Lit): Expr<A extends "f32" ? VecTy : A>;
export function select<B extends NumTy>(cond: In<"bool">, yes: Lit, no: Expr<B>): Expr<B extends "f32" ? VecTy : B>;
export function select<A extends NumTy = "f32", B extends NumTy = "f32">(cond: In<"bool">, yes: Expr<A> | number | Lit, no: Expr<B> | number | Lit): Expr<Wider<A, B>>;
export function select(cond: In<"bool">, yes: In<NumTy>, no: In<NumTy>): Expr {
  return call("select", wider(lift(yes).ty, lift(no).ty), [cond, yes, no]);
}

export const TAU = Math.PI * 2;

// ---- walking the graph --------------------------------------------------------------------------------------------

function children(e: Expr): readonly Expr[] {
  const n = e.node;
  switch (n.op) {
    case "const": case "var": case "input": case "param": return [];
    case "row": return [n.x];
    case "call": case "vec": case "lib": return n.args;
    case "get": return [n.a];
  }
}

/** Every node reachable from the roots, dependencies first. */
export function topo(roots: readonly Expr[]): Expr[] {
  const seen = new Set<number>();
  const out: Expr[] = [];
  const visit = (e: Expr): void => {
    if (seen.has(e.id)) return;
    seen.add(e.id);
    for (const c of children(e)) visit(c);
    out.push(e);
  };
  for (const r of roots) visit(r);
  return out;
}

/** The leaves a set of expressions depends on: what a compiler must bind before emitting them. */
export function leaves(roots: readonly Expr[]): { vars: Map<string, Ty>; inputs: InputSpec[]; params: ParamSpec[]; rows: Row[]; libs: LibFn[] } {
  const vars = new Map<string, Ty>();
  const ins = new Map<string, InputSpec>(), ps = new Map<string, ParamSpec>(), rs = new Map<number, Row>();
  for (const e of topo(roots)) {
    const n = e.node;
    if (n.op === "var") vars.set(n.name, n.ty);
    else if (n.op === "input") ins.set(n.spec.name, n.spec);
    else if (n.op === "param") ps.set(n.spec.name, n.spec);
    else if (n.op === "row") rs.set(n.row.id, n.row);
  }
  return { vars, inputs: [...ins.values()], params: [...ps.values()], rows: [...rs.values()], libs: libsOf(roots) };
}

/** The library functions a set of expressions uses, each after the functions it depends on. */
export function libsOf(roots: readonly Expr[]): LibFn[] {
  const out: LibFn[] = [];
  const seen = new Set<LibFn>();
  const add = (fn: LibFn): void => {
    if (seen.has(fn)) return;
    seen.add(fn);
    for (const d of fn.deps ?? []) add(d);
    out.push(fn);
  };
  for (const e of topo(roots)) if (e.node.op === "lib") add(e.node.fn);
  return out;
}

// ---- emitting code ------------------------------------------------------------------------------------------------

/** How a backend spells the leaves. Each returns source text in the target language. */
export type Bind = {
  variable(name: string, ty: Ty): string;
  input(spec: InputSpec): string;
  param(spec: ParamSpec): string;
  /** A vec4 lookup of `row` at the already-emitted scalar source `x`. */
  row(row: Row, x: string): string;
};

const lit = (n: number): string => (Number.isInteger(n) ? n.toFixed(1) : String(n));
const WGSL_TY: Record<Ty, string> = { f32: "f32", vec2: "vec2<f32>", vec3: "vec3<f32>", vec4: "vec4<f32>", bool: "bool" };

/** Fns whose WGSL form needs every argument to have the result's type (a scalar is splatted to a vector). */
const SPLAT: ReadonlySet<Fn> = new Set<Fn>(["min", "max", "pow", "atan2", "step", "clamp", "smoothstep"]);
const INFIX: Partial<Record<Fn, string>> = { add: "+", sub: "-", mul: "*", div: "/", lt: "<", gt: ">", le: "<=", ge: ">=", and: "&&", or: "||" };

function wgslNode(e: Expr, ref: (x: Expr) => string, bind: Bind): string {
  const n = e.node;
  switch (n.op) {
    case "const":
      if (n.ty === "bool") return n.v[0] ? "true" : "false";
      return n.ty === "f32" ? lit(n.v[0]) : `${WGSL_TY[n.ty]}(${n.v.map(lit).join(", ")})`;
    case "var": return bind.variable(n.name, n.ty);
    case "input": return bind.input(n.spec);
    case "param": return bind.param(n.spec);
    case "row": return bind.row(n.row, ref(n.x));
    case "vec": return `${WGSL_TY[n.ty]}(${n.args.map(ref).join(", ")})`;
    case "get": return `${ref(n.a)}.${n.sel}`;
    case "lib": return `${n.fn.name}(${n.args.map(ref).join(", ")})`;
    case "call": {
      const splat = (a: Expr): string => (SPLAT.has(n.fn) && n.ty !== "f32" && a.ty === "f32" ? `${WGSL_TY[n.ty]}(${ref(a)})` : ref(a));
      const a = n.args.map(splat);
      const infix = INFIX[n.fn];
      if (infix) return `(${a[0]} ${infix} ${a[1]})`;
      switch (n.fn) {
        case "neg": return `(-${a[0]})`;
        case "not": return `(!${a[0]})`;
        case "mod": return `(${a[0]} - ${a[1]} * floor(${a[0]} / ${a[1]}))`;
        case "select": return `select(${a[2]}, ${a[1]}, ${a[0]})`; // WGSL order is (false, true, condition)
        default: return `${n.fn}(${a.join(", ")})`;
      }
    }
  }
}

/**
 * Straight-line WGSL for a set of expressions: one `let` per shared node, in dependency order.
 * Returns the statements, for each root the name (or inline source) that holds its value, and the library
 * functions the statements call.
 */
export function emitWgsl(roots: readonly Expr[], bind: Bind, prefix = "e"): { lines: string[]; out: string[]; libs: LibFn[] } {
  const names = new Map<number, string>();
  const lines: string[] = [];
  const ref = (x: Expr): string => names.get(x.id)!;
  for (const e of topo(roots)) {
    const src = wgslNode(e, ref, bind);
    const leaf = e.node.op === "const" || e.node.op === "var";
    if (leaf) names.set(e.id, src);
    else {
      const name = `${prefix}${e.id}`;
      lines.push(`let ${name}: ${WGSL_TY[e.ty]} = ${src};`);
      names.set(e.id, name);
    }
  }
  // `libs`: definitions the caller must place at module scope (once per module) for the statements to compile.
  return { lines, out: roots.map(ref), libs: libsOf(roots) };
}

// ---- the JavaScript backend ---------------------------------------------------------------------------------------

type V = number | number[];
const map1 = (g: (x: number) => number) => (a: V): V => (typeof a === "number" ? g(a) : a.map(g));
const map2 = (g: (x: number, y: number) => number) => (a: V, b: V): V => {
  if (typeof a === "number") return typeof b === "number" ? g(a, b) : b.map((y) => g(a, y));
  return typeof b === "number" ? a.map((x) => g(x, b)) : a.map((x, i) => g(x, b[i]));
};
const map3 = (g: (x: number, y: number, z: number) => number) => (a: V, b: V, c: V): V => {
  const n = [a, b, c].reduce<number>((m, x) => (typeof x === "number" ? m : x.length), 0);
  const at = (x: V, i: number): number => (typeof x === "number" ? x : x[i]);
  if (n === 0) return g(a as number, b as number, c as number);
  return Array.from({ length: n }, (_, i) => g(at(a, i), at(b, i), at(c, i)));
};
const dotV = (a: V, b: V): number => (typeof a === "number" ? a * (b as number) : a.reduce((s, x, i) => s + x * (b as number[])[i], 0));
const lenV = (a: V): number => Math.sqrt(dotV(a, a));
const smooth = (lo: number, hi: number, x: number): number => {
  const t = Math.min(1, Math.max(0, (x - lo) / (hi - lo)));
  return t * t * (3 - 2 * t);
};

/** The JS runtime the emitted code calls. Semantics match WGSL for every op the WGSL backend emits. */
export const R = {
  add: map2((x, y) => x + y), sub: map2((x, y) => x - y), mul: map2((x, y) => x * y), div: map2((x, y) => x / y),
  min: map2(Math.min), max: map2(Math.max), pow: map2(Math.pow), atan2: map2(Math.atan2),
  mod: map2((x, y) => x - y * Math.floor(x / y)), step: map2((edge, x) => (x >= edge ? 1 : 0)),
  neg: map1((x) => -x), abs: map1(Math.abs), sign: map1(Math.sign), floor: map1(Math.floor),
  fract: map1((x) => x - Math.floor(x)), sqrt: map1(Math.sqrt), exp: map1(Math.exp), log: map1(Math.log),
  sin: map1(Math.sin), cos: map1(Math.cos), tan: map1(Math.tan), saturate: map1((x) => Math.min(1, Math.max(0, x))),
  normalize: (a: V): V => { const l = lenV(a); return typeof a === "number" ? Math.sign(a) : a.map((x) => x / l); },
  mix: map3((a, b, t) => a + (b - a) * t), clamp: map3((x, lo, hi) => Math.min(hi, Math.max(lo, x))), smoothstep: map3(smooth),
  length: lenV, distance: (a: V, b: V): number => lenV(map2((x, y) => x - y)(a, b)), dot: dotV,
  cross: (a: number[], b: number[]): number[] => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]],
  lt: (a: number, b: number): boolean => a < b, gt: (a: number, b: number): boolean => a > b,
  le: (a: number, b: number): boolean => a <= b, ge: (a: number, b: number): boolean => a >= b,
  and: (a: boolean, b: boolean): boolean => a && b, or: (a: boolean, b: boolean): boolean => a || b, not: (a: boolean): boolean => !a,
  select: (c: boolean, yes: V, no: V): V => (c ? yes : no),
  vec: (size: number, ...parts: V[]): number[] => {
    const flat = parts.flatMap((p) => (typeof p === "number" ? [p] : p));
    return flat.length === 1 ? Array.from({ length: size }, () => flat[0]) : flat;
  },
  get: (a: number[], sel: string): V => {
    const picked = [...sel].map((ch) => a["xyzw".indexOf(ch)]);
    return picked.length === 1 ? picked[0] : picked;
  },
  /** Sample a curve row at x: linear interpolation between CURVE_N samples, clamped. */
  row: (samples: Float32Array, x: number): number[] => {
    const s = Math.min(1, Math.max(0, x)) * (CURVE_N - 1);
    const i = Math.min(CURVE_N - 2, Math.floor(s)), t = s - i;
    return [0, 1, 2, 3].map((c) => samples[i * 4 + c] * (1 - t) + samples[(i + 1) * 4 + c] * t);
  },
};

/** What the CPU evaluator reads leaves from. */
export type Env = {
  readonly vars: Readonly<Record<string, V | boolean>>;
  readonly inputs: Readonly<Record<string, V>>;
  readonly params: Readonly<Record<string, number>>;
};

/**
 * Compile expressions to one JavaScript function. Call it with an `Env`; it returns one value per root
 * (a number, an array for vectors, a boolean). Missing inputs and params take their declared defaults.
 */
export function compileJs(roots: readonly Expr[]): (env: Env) => (V | boolean)[] {
  const rows: Row[] = [];
  const fns: LibFn[] = [];
  const body: string[] = [];
  const names = new Map<number, string>();
  const ref = (x: Expr): string => names.get(x.id)!;
  for (const e of topo(roots)) {
    const n = e.node;
    let src: string;
    switch (n.op) {
      case "const": src = n.ty === "bool" ? String(n.v[0] !== 0) : n.v.length === 1 ? String(n.v[0]) : `[${n.v.join(",")}]`; break;
      case "var": src = `vars[${JSON.stringify(n.name)}]`; break;
      case "input": src = `(inputs[${JSON.stringify(n.spec.name)}] ?? ${n.spec.def.length === 1 ? n.spec.def[0] : `[${n.spec.def.join(",")}]`})`; break;
      case "param": src = `(params[${JSON.stringify(n.spec.name)}] ?? ${n.spec.def})`; break;
      case "row": rows.push(n.row); src = `R.row(rows[${rows.length - 1}], ${ref(n.x)})`; break;
      case "vec": src = `R.vec(${SIZE[n.ty]}, ${n.args.map(ref).join(", ")})`; break;
      case "get": src = `R.get(${ref(n.a)}, ${JSON.stringify(n.sel)})`; break;
      case "call": src = `R.${n.fn}(${n.args.map(ref).join(", ")})`; break;
      case "lib":
        if (n.fn.js === "gpu-only") throw new Error(`library function "${n.fn.name}" is GPU-only and cannot be evaluated on the CPU`);
        if (!fns.includes(n.fn)) fns.push(n.fn);
        src = `fns[${fns.indexOf(n.fn)}](${n.args.map(ref).join(", ")})`;
        break;
    }
    const name = `e${e.id}`;
    body.push(`const ${name} = ${src};`);
    names.set(e.id, name);
  }
  body.push(`return [${roots.map(ref).join(", ")}];`);
  const run = new Function("R", "rows", "fns", "vars", "inputs", "params", body.join("\n")) as
    (r: typeof R, rows: Float32Array[], fns: LibFn["js"][], vars: Env["vars"], inputs: Env["inputs"], params: Env["params"]) => (V | boolean)[];
  const samples = rows.map((r) => r.samples);
  const impls = fns.map((fn) => fn.js);
  return (env) => run(R, samples, impls, env.vars, env.inputs, env.params);
}

/** Evaluate once on the CPU. For repeated evaluation, keep the function from `compileJs`. */
export function evaluate(e: Expr, env: Partial<Env> = {}): V | boolean {
  return compileJs([e])({ vars: env.vars ?? {}, inputs: env.inputs ?? {}, params: env.params ?? {} })[0];
}
