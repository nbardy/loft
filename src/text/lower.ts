// Graph functions of WGSL text, lowered into the expression graph per call. A call binds the parameters to the
// caller's expressions and runs the body as a TypeScript function body runs: `let` is a name for a value, `var` and
// assignment are SSA (each assignment a new value), `if` merges what its branches assigned with `select`, `for` over a
// counter is `iterate` with the assigned `var`s as its state, `break` is the loop's stop, `continue` and an early
// `return` mask what follows. Every value is built by the constructors of src/expr.ts, which intern, so the text of a
// function and its call form give the same nodes (tests/text.test.ts checks this by structural hash).
//
// Numbers written in the text stay plain numbers (and vectors of them plain arrays) until an operation needs them, as
// they do in call form: `mix(0.7, 4.6, h.y)` lifts its constants inside `mix`, so the text builds the nodes the call
// form builds, in the same order, and the emitted WGSL is the same text.

import * as X from "../expr.ts";
import { gradient as gradientOf } from "../grad.ts";
import { SceneError, siteOf } from "../errors.ts";
import type { At, ConstItem, Expr as Ast, FnItem, Param, Place, Stmt, StructItem, TypeRef } from "./parse.ts";
import { HOLE_PREFIX } from "./tokens.ts";
import { rec } from "../record/recorder.ts";

type E = X.Expr;
type NumE = X.Expr<X.NumTy>;
type BoolE = X.Expr<"bool">;

// ---- values ---------------------------------------------------------------------------------------------------------

/**
 * What a name holds while a body is lowered.
 * num    a constant number in the text, or one a hole held; `int` while it is an abstract integer literal.
 * lit    a constant vector (a call form's plain array).
 * bool   a constant bool (kept a JavaScript boolean: a node made for it would shift the ids of every node after it).
 * expr   an f32, vector or bool expression.
 * int    an i32 (a loop counter, `let n = 3`): a whole number held as an f32 expression or number.
 * bvec   a vector comparison's result, read by all, any, select, &&, || and !.
 * struct a struct value (a declared struct, or a record a function returned).
 * open   a TypeScript record passed in (a slot's PaintAt, a ray, an object in a hole): fields read on access.
 * array  a constant-length array.
 * mat    a matrix, as its columns.
 * fn     something callable: a prelude function, a function in a hole, a module function.
 */
export type Value =
  | { readonly k: "num"; readonly v: number; readonly int: boolean }
  | { readonly k: "lit"; readonly v: readonly number[] }
  | { readonly k: "bool"; readonly b: boolean }
  | { readonly k: "expr"; readonly e: E }
  | { readonly k: "int"; readonly v: number | X.Expr<"f32"> }
  | { readonly k: "bvec"; readonly parts: readonly B[] }
  | { readonly k: "struct"; readonly name: string; readonly fields: ReadonlyMap<string, Value> }
  | { readonly k: "open"; readonly name: string; readonly obj: Readonly<Record<string, unknown>> }
  | { readonly k: "array"; readonly elems: readonly Value[] }
  | { readonly k: "mat"; readonly cols: readonly Value[] }
  | { readonly k: "fn"; readonly name: string; readonly call: (args: readonly Value[], at: At) => Value };

const num = (v: number, int = false): Value => ({ k: "num", v, int });
const ex = (e: E): Value => ({ k: "expr", e });
/** A bool: a constant, or an expression. */
type B = boolean | BoolE;
const constBool = (b: B): boolean | undefined => (typeof b === "boolean" ? b : undefined);
const fromB = (b: B): Value => (typeof b === "boolean" ? { k: "bool", b } : ex(b));
const liftB = (b: B): BoolE => (typeof b === "boolean" ? X.lift<"bool">(b) : b);
/** A bool value's B, or undefined for a value that is not a bool. */
const asB = (v: Value): B | undefined => (v.k === "bool" ? v.b : v.k === "expr" && v.e.ty === "bool" ? (v.e as BoolE) : undefined);

/** A value's type, as WGSL would name it (for messages and checks). */
export function typeName(v: Value): string {
  switch (v.k) {
    case "num": return v.int ? "an integer literal" : "f32";
    case "bool": return "bool";
    case "lit": return `vec${v.v.length}f`;
    case "expr": return v.e.ty === "f32" || v.e.ty === "bool" ? v.e.ty : `${v.e.ty}f`;
    case "int": return "i32";
    case "bvec": return `vec${v.parts.length}<bool>`;
    case "struct": return v.name;
    case "open": return v.name;
    case "array": return `array of ${v.elems.length}`;
    case "mat": return `mat${v.cols.length}x${v.cols.length}f`;
    case "fn": return `the function ${v.name}`;
  }
}

/** The numeric shape: "f32", "vec2".."vec4", or undefined for anything else (an i32 is not numeric to f32 maths). */
function numTy(v: Value): X.NumTy | undefined {
  switch (v.k) {
    case "num": return "f32";
    case "lit": return (["f32", "f32", "vec2", "vec3", "vec4"] as const)[v.v.length];
    case "expr": return v.e.ty === "bool" ? undefined : v.e.ty;
    default: return undefined;
  }
}

// ---- the module a body is lowered against --------------------------------------------------------------------------

/** Where a module's text came from, for errors and sites. */
export type Source = {
  /** The file the `wgsl` template is in (relative to the library root when inside it), or undefined for `wgsl(text)`. */
  readonly file: string | undefined;
  /** The file line (1-based) of the text's line 0. */
  readonly line0: number;
  /** The `wgsl` call's own site, `file:line`. */
  readonly call: string;
  readonly lines: readonly string[];
};

/** What lowering needs of a module: its source, its items by name, and how to call what is not a graph function. */
export interface Scope0 {
  readonly src: Source;
  readonly fns: ReadonlyMap<string, FnItem>;
  readonly structs: ReadonlyMap<string, StructItem>;
  readonly consts: ReadonlyMap<string, ConstItem>;
  /** A module-scope constant's value (lowered once, when the module is made, in declaration order). */
  constValue(name: string): Value;
  /** A hole's value (`loft_hole_<k>`). */
  hole(k: number): Value;
  /** A call of a raw function from graph code: a GPU-only library call (src/text/index.ts). */
  callRaw(name: string, args: readonly Value[], at: At): Value;
  /** A prelude name's value, or undefined. */
  prelude(name: string): Value | undefined;
  /** Every name in scope at module level, for "did you mean". */
  names(): readonly string[];
}

// ---- errors ---------------------------------------------------------------------------------------------------------

/** A mistake in WGSL text: where (file:line:col), the line quoted with a caret, and the fix. */
export class TextError extends SceneError {
  constructor(src: Source, at: At, given: string, expected: string, fix: string, readonly caller?: string) {
    super(placeOf(src, at), given, expected, fix);
    const text = src.lines[at.line] ?? "";
    const lead = text.length - text.trimStart().length;
    const quote = `  ${text.trim()}\n  ${" ".repeat(Math.max(0, at.col - lead))}^`;
    this.message = `${this.message}\n${quote}${caller === undefined ? "" : `\n  (called from ${caller})`}`;
  }
}
/** `examples/eel.ts:9:22`, or the text's own line and the `wgsl(...)` call for text with no file. */
export function placeOf(src: Source, at: At): string {
  return src.file !== undefined ? `${src.file}:${src.line0 + at.line}:${at.col + 1}` : `line ${at.line + 1} of the wgsl text loaded at ${src.call}`;
}

// ---- sites ----------------------------------------------------------------------------------------------------------

/** Where a node was first built from text: `eel.ts:9 girth`. Kept off the interned key (a node is one node). */
const sites = new WeakMap<E, string>();
export const siteOfNode = (e: E): string | undefined => sites.get(e);

// ---- bindings and scopes --------------------------------------------------------------------------------------------

let nextBinding = 0;
/** A name in a body. `order` is when it was declared: a loop's state is laid out in declaration order. */
type Binding = { value: Value | undefined; readonly mutable: boolean; readonly type: TypeRef | undefined; readonly at: At; readonly order: number };
class Scope {
  readonly vars = new Map<string, Binding>();
  constructor(readonly parent: Scope | undefined) {}
  find(name: string): Binding | undefined { return this.vars.get(name) ?? this.parent?.find(name); }
  declare(name: string, b: Omit<Binding, "order">): Binding { const made = { ...b, order: nextBinding++ }; this.vars.set(name, made); return made; }
  /** Every visible binding by name, the innermost of a shadowed name winning. */
  visible(): Map<string, Binding> {
    const out = this.parent?.visible() ?? new Map<string, Binding>();
    for (const [k, b] of this.vars) out.set(k, b);
    return out;
  }
}

// Control flags, bindings under names WGSL cannot spell: an early return ($returned, and $ret holds the value), and per
// enclosing loop (depth d) a break ($broke<d>) or a continue ($continued<d>) in this iteration.
const RET = "$ret", RETURNED = "$returned";
const BROKE = (d: number) => `$broke${d}`, CONTINUED = (d: number) => `$continued${d}`;

/** One function call being lowered. */
type Frame = { readonly fn: string; readonly ret: TypeRef | undefined; readonly depth: number; readonly stack: readonly string[]; readonly caller: string | undefined };

// ---- the lowerer ----------------------------------------------------------------------------------------------------

/** Each graph function's recording (Lowerer.recording), and the signature types a recorded function may have. */
const recordings = new WeakMap<FnItem, number | string>();
let nextRecorded = 0;
const typeKey = (t: TypeRef): string => (t.args.length === 0 ? t.name : `${t.name}<${t.args.map(typeKey).join(",")}>`);
const NUM_TYPES = new Map<string, X.NumTy>([["f32", "f32"], ["vec2f", "vec2"], ["vec3f", "vec3"], ["vec4f", "vec4"], ["vec2<f32>", "vec2"], ["vec3<f32>", "vec3"], ["vec4<f32>", "vec4"]]);

export class Lowerer {
  constructor(readonly m: Scope0) {}

  fail(at: At, given: string, expected: string, fix: string, caller?: string): never {
    throw new TextError(this.m.src, at, given, expected, fix, caller);
  }

  // ---- calls ----

  /**
   * Call the graph function `name` with `args` (already values). `caller` is the TypeScript site that called into the
   * text, quoted by errors that depend on the arguments.
   */
  call(item: FnItem & { kind: "graph" }, args: readonly Value[], at: At, stack: readonly string[], caller: string | undefined): Value {
    // Recorded once over binders and per call as one `Call` (src/record/recorder.ts) when the call is a function of
    // its arguments' values alone: every argument a non-constant expression (a constant would fold differently in
    // the body), and no identities open (they fold differently too). The inlining below is then for the TS engine only.
    const fn = args.every((a) => a.k === "expr" && a.e.node.op !== "const") && !X.identitiesOn() ? this.recording(item) : "a constant argument or identities";
    if (typeof fn === "string") return this.inline(item, args, at, stack, caller);
    const since = rec.size, out = rec.muted(() => this.inline(item, args, at, stack, caller));
    if (out.k === "expr") rec.call(fn, args.map((a) => (a as Value & { k: "expr" }).e), out.e, since);
    return out;
  }

  /** `item`'s recording: its number, or why each call of it is inlined (one classification per function). */
  recording(item: FnItem & { kind: "graph" }): number | string {
    let r = recordings.get(item);
    if (r !== undefined) return r;
    r = this.unrecordable(item, new Set()) ?? rec.fn(item.sig.name, () => {
      const id = nextRecorded++, binders = item.sig.params.map((p, j) => X.variable(`loft_fn${id}_${j}`, NUM_TYPES.get(typeKey(p.type))!));
      try {
        const out = this.inline(item, binders.map(ex), item.sig.at, [], undefined);
        return out.k === "expr" ? { binders, body: out.e } : `${item.sig.name} returns a constant`;
      } catch (e) {
        // Only an authoring refusal over binders (say, a loop bound that must be constant) is a reason to inline; any
        // other throw is a bug and surfaces.
        if (!(e instanceof TextError)) throw e;
        return `${item.sig.name} does not lower over binders: ${e.message}`;
      }
    });
    recordings.set(item, r);
    return r;
  }

  /** Why `item` cannot be one recorded body for every call, or undefined: read from its text, through the functions it calls. */
  unrecordable(item: FnItem & { kind: "graph" }, seen: Set<FnItem>): string | undefined {
    seen.add(item);
    // A bool argument may be a `not` the body would simplify away (boolNot); a vector is fine.
    const typed = [...item.sig.params.map((p) => p.type), ...(item.sig.ret === undefined ? [] : [item.sig.ret])].find((t) => !NUM_TYPES.has(typeKey(t)));
    if (typed !== undefined) return `${item.sig.name} has a ${typeKey(typed)} in its signature`;
    const names: string[] = [];
    const scan = (x: unknown): void => {
      if (Array.isArray(x)) for (const y of x) scan(y);
      else if (typeof x === "object" && x !== null) {
        const o = x as { k?: string; name?: string };
        if (o.k === "ident" && o.name !== undefined) names.push(o.name);
        for (const v of Object.values(o)) scan(v);
      }
    };
    scan(item.body);
    for (const n of names) {
      // gradient reads which arguments are leaves; a hole holding a function or a record may run TypeScript that does.
      if (n === "gradient") return `${item.sig.name} reads gradient`;
      if (n.startsWith(HOLE_PREFIX)) {
        const k = this.m.hole(Number(n.slice(HOLE_PREFIX.length))).k;
        if (k === "fn" || k === "open") return `${item.sig.name} reads a hole holding a ${k === "fn" ? "function" : "record"}`;
      }
      const f = this.m.fns.get(n);
      if (f?.kind === "graph" && !seen.has(f)) { const why = this.unrecordable(f, seen); if (why !== undefined) return why; }
    }
    return undefined;
  }

  inline(item: FnItem & { kind: "graph" }, args: readonly Value[], at: At, stack: readonly string[], caller: string | undefined): Value {
    const sig = item.sig;
    if (stack.includes(sig.name)) this.fail(at, `${sig.name} calling itself (${[...stack, sig.name].join(" -> ")})`, "no recursion (WGSL has none)", "write the repetition as a for loop");
    if (args.length !== sig.params.length) this.fail(at, `${args.length} argument${args.length === 1 ? "" : "s"} to ${sig.name}`, `${sig.params.length} (${sig.params.map((p) => p.name).join(", ")})`, `call ${sig.name}(${sig.params.map((p) => p.name).join(", ")})`, caller);
    const scope = new Scope(undefined);
    sig.params.forEach((p, i) => scope.declare(p.name, { value: this.conform(args[i]!, p.type, p.at, `${sig.name}'s parameter ${p.name}`, caller), mutable: false, type: p.type, at: p.at }));
    scope.declare(RETURNED, { value: fromB(false), mutable: true, type: undefined, at: sig.at });
    scope.declare(RET, { value: undefined, mutable: true, type: sig.ret, at: sig.at });
    const frame: Frame = { fn: sig.name, ret: sig.ret, depth: 0, stack: [...stack, sig.name], caller };
    this.block(item.body, scope, frame);
    if (sig.ret === undefined) this.fail(sig.at, `${sig.name}, which returns nothing`, "a function that returns a value (graph code is pure: a call is its value)", `give it a return type: fn ${sig.name}(...) -> f32`);
    // WGSL's rule, read from the statements (not from the flags, whose algebra need not simplify to false).
    if (!alwaysReturns(item.body)) this.fail(sig.at, `${sig.name}, which can reach its end without returning`, "a return on every path", "end the function with a return");
    return scope.find(RET)!.value!;
  }

  // ---- statements ----

  block(body: readonly Stmt[], scope: Scope, frame: Frame): void {
    for (const s of body) {
      if (constBool(this.live(scope, frame)) === false) return; // after an unconditional return, break or continue
      this.statement(s, scope, frame);
    }
  }

  statement(s: Stmt, scope: Scope, frame: Frame): void {
    switch (s.k) {
      case "let": case "const": case "var": {
        if (s.name.startsWith(HOLE_PREFIX)) this.fail(s.at, "a hole as a name", "an identifier", "name it in the text");
        let value = s.init === undefined ? undefined : this.expr(s.init, scope, frame);
        if (value !== undefined && s.type !== undefined) value = this.conform(value, s.type, s.at, s.name);
        // WGSL makes `let n = 1` an i32: an abstract integer is concretized when it is named.
        if (value?.k === "num" && value.int) value = { k: "int", v: value.v };
        // A var holds an expression from its declaration, as a call form's `let x: F = f(0)` does (its constant is made
        // where the var is declared, so a loop's state starts in the call form's order).
        if (s.k === "var" && (value?.k === "num" || value?.k === "lit")) value = ex(X.lift(callArg(value) as X.In));
        if (value?.k === "fn" && s.k === "var") this.fail(s.at, `a var holding ${typeName(value)}`, "a value", `use let ${s.name}`);
        scope.declare(s.name, { value, mutable: s.k === "var", type: s.type, at: s.at });
        return;
      }
      case "assign": return this.assign(s.place, s.op, s.value, scope, frame, s.at);
      case "step": {
        const b = this.binding(s.place.name, scope, s.at);
        const old = this.read(s.place, scope, frame);
        if (old.k !== "int") this.fail(s.at, `${s.place.name}${s.op} on ${typeName(old)}`, "an integer (WGSL's ++ and -- take i32 or u32 only)", `${s.place.name} ${s.op === "++" ? "+=" : "-="} 1.0;`);
        this.write(b, s.place, this.arith(s.op === "++" ? "+" : "-", old, num(1, true), s.at), scope, frame);
        return;
      }
      case "if": return this.ifStatement(s, scope, frame);
      case "for": return this.forStatement(s, scope, frame);
      case "break": case "continue": {
        if (frame.depth === 0) this.fail(s.at, `${s.k} outside a loop`, "a for loop around it", `delete the ${s.k}`);
        this.setFlag(scope, frame, s.k === "break" ? BROKE(frame.depth - 1) : CONTINUED(frame.depth - 1));
        return;
      }
      case "return": {
        if (s.value === undefined) this.fail(s.at, "return with no value", "return <value>", "return the function's result");
        const v = this.conform(this.expr(s.value, scope, frame), frame.ret!, s.at, `${frame.fn}'s result`, frame.caller);
        this.assignBinding(scope.find(RET)!, v, scope, frame);
        this.setFlag(scope, frame, RETURNED);
        return;
      }
      case "block": { this.block(s.body, new Scope(scope), frame); return; }
      case "call": this.fail(s.at, "a call whose value is not used", "a statement with an effect (graph code is pure)", "assign the call's value, or delete the line");
    }
  }

  binding(name: string, scope: Scope, at: At): Binding {
    const b = scope.find(name);
    if (b === undefined) this.fail(at, `an assignment to ${name}, which is not declared here`, "a var declared in this function", `var ${name} = ...;`);
    if (!b.mutable) this.fail(at, `an assignment to ${name}, declared with let`, "a var", `declare it var ${name} = ...;`);
    return b;
  }

  assign(place: Place, op: string, value: Ast, scope: Scope, frame: Frame, at: At): void {
    const b = this.binding(place.name, scope, at);
    let v = this.expr(value, scope, frame);
    if (op !== "=") v = this.arith(op.slice(0, -1), this.read(place, scope, frame), v, at);
    this.write(b, place, v, scope, frame);
  }

  /** The value at a place now. */
  read(place: Place, scope: Scope, frame: Frame): Value {
    let v = this.lookup(place.name, scope, frame, place.at);
    for (const p of place.path) v = p.k === "member" ? this.member(v, p.name, place.at) : this.index(v, this.expr(p.i, scope, frame), place.at);
    return v;
  }

  /** Write `v` at `place` of binding `b`: a field or component write rebuilds the value around it. */
  write(b: Binding, place: Place, v: Value, scope: Scope, frame: Frame): void {
    const put = (old: Value, path: Place["path"], v: Value): Value => {
      if (path.length === 0) return b.type !== undefined && place.path.length === 0 ? this.conform(v, b.type, place.at, place.name) : this.sameShape(old, v, place);
      const [p, ...rest] = path;
      if (p!.k === "index") {
        const i = this.expr(p!.i, scope, frame);
        if (old.k === "array") {
          const at = this.whole(i, place.at);
          if (at === undefined) this.fail(place.at, "a write at an index that is not a constant", "a constant index (a for loop's counter is an expression)", "build the array with the new value instead");
          return { k: "array", elems: old.elems.map((e, j) => (j === at ? put(e, rest, v) : e)) };
        }
        const at = this.whole(i, place.at);
        if (at === undefined || numTy(old) === undefined || numTy(old) === "f32") this.fail(place.at, `an index write into ${typeName(old)}`, "a vector at a constant index", "assign the whole value");
        return put(old, [{ k: "member", name: "xyzw"[at]! }, ...rest], v);
      }
      if (old.k === "struct") {
        if (!old.fields.has(p!.name)) this.fail(place.at, `.${p!.name} of ${old.name}`, `one of its fields (${[...old.fields.keys()].join(", ")})`, "use a field the struct declares");
        const fields = new Map(old.fields);
        fields.set(p!.name, put(old.fields.get(p!.name)!, rest, v));
        return { k: "struct", name: old.name, fields };
      }
      const ty = numTy(old);
      if (ty === undefined || ty === "f32") this.fail(place.at, `a component write into ${typeName(old)}`, "a vector", "assign the whole value");
      if (rest.length > 0) this.fail(place.at, "a write through a swizzle", "one swizzle", "assign the components one level up");
      const sel = this.swizzleOf(p!.name, ty, place.at);
      const n = Number(ty.slice(3));
      if (new Set(sel).size !== sel.length) this.fail(place.at, `.${p!.name}, which names a component twice`, "each component once", "write each component once");
      const parts = Array.from({ length: n }, (_, c) => {
        const j = sel.indexOf("xyzw"[c]!);
        return j < 0 ? this.component(old, c, place.at) : sel.length === 1 ? v : this.component(v, j, place.at);
      });
      if (sel.length > 1 && numTy(v) !== `vec${sel.length}`) this.fail(place.at, `${typeName(v)} into .${p!.name}`, `vec${sel.length}f`, `give ${sel.length} components`);
      if (sel.length === 1 && numTy(v) !== "f32") this.fail(place.at, `${typeName(v)} into .${p!.name}`, "f32", "give one number");
      return this.construct(ty as X.VecTy, parts, place.at);
    };
    const old = b.value;
    if (old === undefined && place.path.length > 0) this.fail(place.at, `a write into ${place.name} before it has a value`, "a value first", `${place.name} = ...;`);
    this.assignBinding(b, old === undefined ? (b.type !== undefined ? this.conform(v, b.type, place.at, place.name) : v) : put(old, place.path, v), scope, frame);
  }

  /** A whole value written over `old` keeps its type (WGSL's var has one type). */
  sameShape(old: Value, v: Value, place: Place): Value {
    const a = this.shapeOf(old), b = this.shapeOf(v);
    if (a !== b) {
      if (old.k === "int" && v.k === "num" && v.int) return { k: "int", v: v.v };
      if (a === "f32" && v.k === "num") return num(v.v);
      this.fail(place.at, `${typeName(v)} assigned to ${place.name}, a ${typeName(old)}`, typeName(old), old.k === "int" ? `write ${place.name} = f32(...) in a new var, or keep it an integer` : `assign a ${typeName(old)}`);
    }
    return v.k === "num" && v.int ? num(v.v) : v;
  }
  shapeOf(v: Value): string {
    switch (v.k) {
      case "num": return v.int ? "abstract" : "f32";
      case "int": return "i32";
      case "struct": return v.name;
      default: return numTy(v) ?? typeName(v);
    }
  }

  /** Assign under the current flags: where an earlier return, break or continue may have run, the old value stays there. */
  assignBinding(b: Binding, v: Value, scope: Scope, frame: Frame): void {
    const live = this.live(scope, frame);
    const c = constBool(live);
    if (c === false) return;
    if (c === true || b.value === undefined) { b.value = v; return; }
    b.value = this.merge(live, v, b.value);
  }

  setFlag(scope: Scope, frame: Frame, name: string): void {
    this.assignBinding(scope.find(name)!, fromB(true), scope, frame);
  }

  /** Whether execution reaches here: no return, and no break or continue in an enclosing loop this iteration. */
  live(scope: Scope, frame: Frame): B {
    let any: B = false;
    const add = (name: string): void => { const v = scope.find(name)?.value; const b = v === undefined ? undefined : asB(v); if (b !== undefined) any = boolOr(any, b); };
    add(RETURNED);
    for (let d = 0; d < frame.depth; d++) { add(BROKE(d)); add(CONTINUED(d)); }
    return boolNot(any);
  }

  /** `yes` where `cond` holds, else `no`, for any two values of one shape. */
  merge(cond: B, yes: Value, no: Value): Value {
    if (sameValue(yes, no)) return yes;
    const c = constBool(cond);
    if (c !== undefined) return c ? yes : no;
    if (yes.k === "struct" && no.k === "struct") {
      const fields = new Map<string, Value>();
      for (const [k, v] of yes.fields) fields.set(k, this.merge(cond, v, no.fields.get(k)!));
      return { k: "struct", name: yes.name, fields };
    }
    if (yes.k === "array" && no.k === "array") return { k: "array", elems: yes.elems.map((v, i) => this.merge(cond, v, no.elems[i]!)) };
    if (yes.k === "mat" && no.k === "mat") return { k: "mat", cols: yes.cols.map((v, i) => this.merge(cond, v, no.cols[i]!)) };
    const c0 = cond as BoolE;
    if (yes.k === "int" && no.k === "int") return { k: "int", v: select(c0, yes.v, no.v) as X.Expr<"f32"> };
    const yb = asB(yes), nb = asB(no);
    if (yb !== undefined && nb !== undefined) return fromB(boolSelect(c0, yb, nb));
    if (yes.k === "bvec" && no.k === "bvec") return { k: "bvec", parts: yes.parts.map((p, i) => boolSelect(c0, p, no.parts[i]!)) };
    const a = callArg(yes), b = callArg(no);
    if (a === undefined || b === undefined) throw new Error(`internal: text: cannot merge ${typeName(yes)} with ${typeName(no)}`);
    return ex(select(c0, a, b));
  }

  ifStatement(s: Stmt & { k: "if" }, scope: Scope, frame: Frame): void {
    const cv = this.expr(s.cond, scope, frame);
    const cond = asB(cv);
    if (cond === undefined) return this.fail(s.cond.at, `an if condition of ${typeName(cv)}`, "a bool", "compare it: if (x > 0.0)");
    const known = constBool(cond);
    if (known !== undefined) { const b = known ? s.then : s.else; if (b) this.block(b, new Scope(scope), frame); return; }
    const all = scope.visible();
    const before = new Map<Binding, Value | undefined>();
    for (const b of all.values()) before.set(b, b.value);
    this.block(s.then, new Scope(scope), frame);
    const after = new Map<Binding, Value | undefined>();
    for (const b of before.keys()) { after.set(b, b.value); b.value = before.get(b); }
    if (s.else) this.block(s.else, new Scope(scope), frame);
    // Each name either branch changed: the then value where the condition holds. Values in the branches were already
    // masked by the flags, so the merge needs no mask of its own.
    for (const [b, t] of after) {
      const e = b.value;
      if (t === e) continue;
      b.value = t === undefined ? e : e === undefined ? t : this.merge(cond, t, e);
    }
  }

  forStatement(s: Stmt & { k: "for" }, scope: Scope, frame: Frame): void {
    // The counter: for (var i = START; i < END; i++) with START an integer and END an integer or i32(expression).
    const init = s.init, cond = s.cond, update = s.update;
    const shape = "for (var i = 0; i < N; i++), with N a whole number or i32(expression)";
    if (init?.k !== "var" || init.init === undefined) return this.fail(s.at, "a for loop without a counter", shape, "count the loop with an integer: for (var i = 0; i < 8; i++)");
    const counter = init.name;
    const start = this.expr(init.init, scope, frame);
    if (!(start.k === "num" && start.int)) this.fail(init.at, `a loop counter starting at ${typeName(start)}`, "an integer literal", `var ${counter} = 0`);
    if (cond?.k !== "binary" || cond.a.k !== "ident" || cond.a.name !== counter || (cond.op !== "<" && cond.op !== "<=")) this.fail(cond?.at ?? s.at, "this loop condition", `${counter} < N or ${counter} <= N`, shape);
    const okUpdate = (update?.k === "step" || update?.k === "assign") && update.place.name === counter && update.place.path.length === 0 && (
      (update.k === "step" && update.op === "++") ||
      (update.k === "assign" && update.op === "+=" && update.value.k === "num" && update.value.v === 1) ||
      (update.k === "assign" && update.op === "=" && update.value.k === "binary" && update.value.op === "+" && update.value.a.k === "ident" && update.value.a.name === counter && update.value.b.k === "num" && update.value.b.v === 1));
    if (!okUpdate) this.fail(update?.at ?? s.at, "this loop step", `${counter}++`, shape);
    const end = this.loopEnd(cond.b, scope, frame);
    const s0 = (start as { v: number }).v;
    const span: number | X.Expr<"f32"> = typeof end === "number" ? end - s0 + (cond.op === "<=" ? 1 : 0) : (s0 === 0 && cond.op === "<" ? end : X.add(end, (cond.op === "<=" ? 1 : 0) - s0));
    const count: number | X.Expr<"f32"> = typeof span === "number" ? Math.max(0, span) : span;

    // The state: every outer var the body assigns (in declaration order), and an early return's value and flag.
    const assigned = assignedNames(s.body, new Set([counter]));
    if (assigned.has(counter)) this.fail(s.at, `an assignment to the counter ${counter} in the body`, "a counter only the loop steps", "keep a separate var");
    const returns = containsReturn(s.body);
    const visible = scope.visible();
    const names = [...assigned].filter((n) => visible.has(n));
    for (const n of assigned) if (!visible.has(n) || !visible.get(n)!.mutable) this.binding(n, scope, s.at);
    if (returns) names.push(RET, RETURNED);
    names.sort((a, b) => visible.get(a)!.order - visible.get(b)!.order);
    const layouts = names.map((n) => {
      const b = visible.get(n)!;
      const v = b.value ?? (b.type !== undefined ? zeroOf(b.type, this, b.at) : this.fail(s.at, `${n}, which has no value before the loop`, "a value or a type", `var ${n}: f32 = 0.0;`));
      return { name: n, layout: layoutOf(v, this, s.at, n) };
    });
    const init0 = layouts.flatMap(({ name, layout }) => layout.flatten(visible.get(name)!.value ?? zeroOf(visible.get(name)!.type!, this, s.at)));

    // The step re-lowers the body with fresh variables for the state, from a snapshot of the scope at the loop: it is
    // called again by `replace`, `fold` and `grad` (src/expr.ts, src/grad.ts) long after this statement, and must not
    // read bindings that have changed since.
    const frozen = new Map<string, Binding>();
    for (const [k, b] of visible) frozen.set(k, { ...b });
    const depth = frame.depth;
    let stop: B | undefined;
    const step = (state: readonly NumE[], i: X.Expr<"f32">): NumE[] => {
      const outer = new Scope(undefined);
      for (const [k, b] of frozen) outer.vars.set(k, { ...b });
      let at = 0;
      for (const { name, layout } of layouts) { outer.vars.get(name)!.value = layout.rebuild(state.slice(at, at + layout.size)); at += layout.size; }
      const own = new Scope(outer);
      own.declare(counter, { value: { k: "int", v: s0 === 0 ? i : X.add(i, s0) }, mutable: false, type: undefined, at: init.at });
      own.declare(BROKE(depth), { value: fromB(false), mutable: true, type: undefined, at: s.at });
      own.declare(CONTINUED(depth), { value: fromB(false), mutable: true, type: undefined, at: s.at });
      const inner: Frame = { ...frame, depth: depth + 1 };
      this.block(s.body, new Scope(own), inner);
      const broke = asB(own.vars.get(BROKE(depth))!.value!)!;
      const returned = asB(outer.vars.get(RETURNED)!.value!)!;
      // `until` below hands iterate this step's stop: iterate calls `until` right after `step`, and so do `replace`,
      // `fold` and grad's loop tangents, so the stop always belongs to the step just taken.
      stop = returns ? boolOr(broke, returned) : broke;
      return layouts.flatMap(({ name, layout }) => layout.flatten(outer.vars.get(name)!.value!));
    };
    const stops = containsBreak(s.body) || returns;
    const results = X.iterate(count, init0 as never, step as never, stops ? ((() => liftB(stop!)) as never) : undefined) as unknown as NumE[];
    let at = 0;
    for (const { name, layout } of layouts) { visible.get(name)!.value = layout.rebuild(results.slice(at, at + layout.size)); at += layout.size; }
  }

  /** A loop's end: an integer literal, or i32(expression) (an expression count, truncated as `iterate` truncates). */
  loopEnd(e: Ast, scope: Scope, frame: Frame): number | X.Expr<"f32"> {
    if (e.k === "call" && e.callee.k === "ident" && e.callee.name === "i32" && e.args.length === 1) {
      const v = this.expr(e.args[0]!, scope, frame);
      if (v.k === "num") return Math.trunc(v.v);
      if (v.k === "expr" && v.e.ty === "f32") return v.e as X.Expr<"f32">;
      if (v.k === "int") return v.v;
      this.fail(e.at, `i32 of ${typeName(v)}`, "an f32", "count with a number");
    }
    const v = this.expr(e, scope, frame);
    if (v.k === "num" && v.int) return v.v;
    if (v.k === "int") return v.v;
    return this.fail(e.at, `a loop end of ${typeName(v)}`, "an integer literal, or i32(expression)", `i < i32(n)`);
  }

  // ---- expressions ----

  expr(a: Ast, scope: Scope, frame: Frame): Value {
    const v = this.expr0(a, scope, frame);
    if (v.k === "expr" && !sites.has(v.e)) sites.set(v.e, `${placeOf(this.m.src, a.at).replace(/:\d+$/, "")} ${frame.fn}`);
    return v;
  }

  expr0(a: Ast, scope: Scope, frame: Frame): Value {
    switch (a.k) {
      case "num": return num(a.v, a.int);
      case "bool": return fromB(a.v);
      case "ident": return this.lookup(a.name, scope, frame, a.at);
      case "unary": {
        const v = this.expr(a.a, scope, frame);
        if (a.op === "-") {
          switch (v.k) {
            case "num": return num(-v.v, v.int);
            case "lit": return { k: "lit", v: v.v.map((x) => -x) };
            case "int": return { k: "int", v: typeof v.v === "number" ? -v.v : X.neg(v.v) };
            case "expr": if (v.e.ty !== "bool") return ex(X.neg(v.e as NumE)); break;
            case "mat": return { k: "mat", cols: v.cols.map((c) => this.arith("*", c, num(-1), a.at)) };
            default: break;
          }
          this.fail(a.at, `-${typeName(v)}`, "a number or vector", "negate a number");
        }
        const vb = asB(v);
        if (vb !== undefined) return fromB(boolNot(vb));
        if (v.k === "bvec") return { k: "bvec", parts: v.parts.map(boolNot) };
        return this.fail(a.at, `!${typeName(v)}`, "a bool", "compare first: !(x > 0.0)");
      }
      case "binary": return this.binary(a.op, this.expr(a.a, scope, frame), this.expr(a.b, scope, frame), a.at);
      case "member": return this.member(this.expr(a.a, scope, frame), a.name, a.at);
      case "index": return this.index(this.expr(a.a, scope, frame), this.expr(a.i, scope, frame), a.at);
      case "call": return this.callExpr(a, scope, frame);
    }
  }

  lookup(name: string, scope: Scope, frame: Frame, at: At): Value {
    const b = scope.find(name);
    if (b !== undefined) {
      if (b.value === undefined) this.fail(at, `${name}, read before it has a value`, "a value first", `var ${name}: f32 = 0.0;`);
      return b.value;
    }
    if (name.startsWith(HOLE_PREFIX)) return this.m.hole(Number(name.slice(HOLE_PREFIX.length)));
    if (this.m.consts.has(name)) return this.m.constValue(name);
    if (name === "time") return ex(X.time);
    const fn = this.m.fns.get(name);
    if (fn !== undefined) return { k: "fn", name, call: (args, cat) => this.callItem(fn, args, cat, frame) };
    const p = this.m.prelude(name);
    if (p !== undefined) return p;
    return this.unknown(name, scope, at);
  }

  unknown(name: string, scope: Scope, at: At): never {
    const near = closest(name, [...scope.visible().keys()].filter((k) => !k.startsWith("$")).concat(this.m.names(), BUILTIN_NAMES));
    this.fail(at, `unknown name "${name}"`, "a name in scope (a local, a function or struct of this module, a hole, time, or a prelude function)", near === undefined ? `declare ${name}, or bring it in through a hole: \${${name}}` : `did you mean ${near}?`);
  }

  constant(c: ConstItem): Value {
    let v = this.expr(c.init, new Scope(undefined), { fn: c.name, ret: undefined, depth: 0, stack: [c.name], caller: undefined });
    if (c.type !== undefined) v = this.conform(v, c.type, c.at, c.name);
    return v;
  }

  callItem(fn: FnItem, args: readonly Value[], at: At, frame: Frame): Value {
    switch (fn.kind) {
      case "graph": return this.call(fn, args, at, frame.stack, frame.caller);
      case "raw": return this.m.callRaw(fn.sig.name, args, at);
      case "entry": return this.fail(at, `a call of the entry point ${fn.sig.name}`, "a function", "entry points are run by draw.shader or compute; call a plain fn");
    }
  }

  callExpr(a: Ast & { k: "call" }, scope: Scope, frame: Frame): Value {
    const at = a.at;
    if (a.callee.k === "ident") {
      const name = a.callee.name;
      // Locals and module items first (a module function may shadow a builtin, as in WGSL); then constructors and builtins.
      if (scope.find(name) === undefined && !name.startsWith(HOLE_PREFIX) && !this.m.fns.has(name)) {
        if (name === "gradient") return this.gradient(a, scope, frame);
        const st = this.m.structs.get(name) ?? PRELUDE_STRUCTS.get(name);
        if (st !== undefined) return this.structOf(st.name, st.fields, a.args.map((x) => this.expr(x, scope, frame)), at);
        const ctor = this.ctor(name, a.type, a.args.map((x) => this.expr(x, scope, frame)), at);
        if (ctor !== undefined) return ctor;
        const builtin = BUILTINS.get(name);
        if (builtin !== undefined) return builtin(this, a.args.map((x) => this.expr(x, scope, frame)), at);
      }
    }
    const callee = this.expr(a.callee, scope, frame);
    if (callee.k !== "fn") return this.fail(at, `a call of ${typeName(callee)}`, "a function", "call a function");
    return callee.call(a.args.map((x) => this.expr(x, scope, frame)), at);
  }

  /** gradient(h, p): h's gradient with respect to the value p names (a vec2 or vec3), through every use of p in h. */
  gradient(a: Ast & { k: "call" }, scope: Scope, frame: Frame): Value {
    if (a.args.length !== 2 || a.args[1]!.k !== "ident") this.fail(a.at, "gradient with these arguments", "gradient(h, p): a scalar expression and the name of a vec2 or vec3", "gradient(height(p), p)");
    const h = this.expr(a.args[0]!, scope, frame), p = this.expr(a.args[1]!, scope, frame);
    const pt = numTy(p);
    if (pt !== "vec2" && pt !== "vec3") this.fail(a.args[1]!.at, `the gradient's point of ${typeName(p)}`, "a vec2 or vec3", "differentiate with respect to a point");
    if (numTy(h) !== "f32") this.fail(a.args[0]!.at, `the gradient of ${typeName(h)}`, "an f32", "take the gradient of a scalar");
    if (h.k !== "expr") return { k: "lit", v: Array(Number(pt.slice(3))).fill(0) };
    const pe = X.lift(callArg(p)! as X.In) as NumE;
    const leaf = pe.node.op === "var" || pe.node.op === "input" || pe.node.op === "param";
    if (leaf) return ex(gradientOf(h.e as X.Expr<"f32">, pe as X.Expr<"vec3">));
    // p is a computed value: differentiate with respect to a fresh variable standing for it, then put p back.
    const v = X.variable(`loft_grad${pe.id}`, pt);
    const [hv] = X.replace([h.e], new Map([[pe.id, v]]));
    const g = gradientOf(hv as X.Expr<"f32">, v as X.Expr<"vec3">);
    return ex(X.replace([g], new Map([[v.id, pe]]))[0]!);
  }

  // ---- operators ----

  binary(op: string, a: Value, b: Value, at: At): Value {
    switch (op) {
      case "+": case "-": case "*": case "/": case "%": return this.arith(op, a, b, at);
      case "&&": case "||": {
        if (a.k === "bvec" && b.k === "bvec" && a.parts.length === b.parts.length) return { k: "bvec", parts: a.parts.map((p, i) => (op === "&&" ? boolAnd : boolOr)(p, b.parts[i]!)) };
        const x = asB(a), y = asB(b);
        if (x === undefined || y === undefined) return this.fail(at, `${typeName(a)} ${op} ${typeName(b)}`, "two bools", "compare first: (a > 0.0) && (b < 1.0)");
        // Both sides are always evaluated (the graph is pure); a constant side folds, as WGSL folds it.
        if (typeof x === "boolean" || typeof y === "boolean") return fromB(op === "&&" ? boolAnd(x, y) : boolOr(x, y));
        return ex(op === "&&" ? X.and(x, y) : X.or(x, y));
      }
      case "<": case ">": case "<=": case ">=": case "==": case "!=": return this.compare(op, a, b, at);
      default: return this.fail(at, `the operator ${op}`, "an operator of the subset", "bit operations make a function raw; this one was not caught: report it");
    }
  }

  compare(op: string, a: Value, b: Value, at: At): Value {
    const fn = ({ "<": X.lt, ">": X.gt, "<=": X.le, ">=": X.ge, "==": X.eq, "!=": X.ne } as const)[op as "<"]!;
    const ab = asB(a), bb = asB(b);
    if (ab !== undefined && bb !== undefined && (op === "==" || op === "!=")) {
      const same = boolOr(boolAnd(ab, bb), boolAnd(boolNot(ab), boolNot(bb)));
      return fromB(op === "==" ? same : boolNot(same));
    }
    const ints = (a.k === "int" || (a.k === "num" && a.int)) && (b.k === "int" || (b.k === "num" && b.int));
    if (ints) {
      const x = a.k === "int" ? a.v : (a as { v: number }).v, y = b.k === "int" ? b.v : (b as { v: number }).v;
      if (typeof x === "number" && typeof y === "number") return fromB(cmpNum(op, x, y));
      return ex(fn(x, y));
    }
    if (a.k === "int" || b.k === "int") this.intMix(a.k === "int" ? b : a, at);
    const ta = numTy(a), tb = numTy(b);
    if (ta === undefined || tb === undefined) this.fail(at, `${typeName(a)} ${op} ${typeName(b)}`, "numbers or vectors", "compare numbers");
    if (ta === "f32" && tb === "f32") {
      if (a.k === "num" && b.k === "num") return fromB(cmpNum(op, a.v, b.v));
      return ex(fn(callArg(a) as X.In<"f32">, callArg(b) as X.In<"f32">));
    }
    if (ta !== tb) this.fail(at, `${typeName(a)} ${op} ${typeName(b)}`, "two values of one type", "compare vectors of one size, or components");
    const n = Number(ta.slice(3));
    // A vector comparison: per component, kept scalar so robustness margins stay exact (all() and any() join them).
    return { k: "bvec", parts: Array.from({ length: n }, (_, i) => fn(callArg(this.component(a, i, at)) as never, callArg(this.component(b, i, at)) as never)) };
  }

  intMix(other: Value, at: At): never {
    return this.fail(at, `an i32 with ${typeName(other)}`, "two i32s, or two f32s", "convert the integer: f32(i) (and write float literals as 1.0)");
  }

  /** + - * / % on two values (folded when both are constants, as WGSL evaluates constant expressions). */
  arith(op: string, a: Value, b: Value, at: At): Value {
    if (a.k === "mat" || b.k === "mat") return this.matArith(op, a, b, at);
    const aInt = a.k === "int" || (a.k === "num" && a.int), bInt = b.k === "int" || (b.k === "num" && b.int);
    if ((a.k === "int" || b.k === "int") && aInt && bInt) {
      if (op === "/" || op === "%") this.fail(at, `integer ${op}`, "f32 maths (integer division and remainder are raw until phase 2)", `f32(${"i"}) ${op} 2.0`);
      const x = a.k === "int" ? a.v : (a as { v: number }).v, y = b.k === "int" ? b.v : (b as { v: number }).v;
      if (typeof x === "number" && typeof y === "number") return { k: "int", v: foldNum(op, x, y, true) };
      return { k: "int", v: ({ "+": X.add, "-": X.sub, "*": X.mul } as const)[op as "+"]!(x as never, y as never) as X.Expr<"f32"> };
    }
    if (a.k === "int") this.intMix(b, at);
    if (b.k === "int") this.intMix(a, at);
    const ta = numTy(a), tb = numTy(b);
    if (ta === undefined || tb === undefined) this.fail(at, `${typeName(a)} ${op} ${typeName(b)}`, "numbers or vectors", "do arithmetic on numbers and vectors");
    if (ta !== "f32" && tb !== "f32" && ta !== tb) this.fail(at, `${typeName(a)} ${op} ${typeName(b)}`, "operands of one size (or a scalar, which goes with any)", `make both ${ta > tb ? ta : tb}s, or take .${"xyzw".slice(0, Math.min(Number(ta.slice(3)), Number(tb.slice(3))))} of the larger`);
    // Constants fold, as WGSL evaluates a constant expression: `0.5 * 3.0` is the number 1.5 (and a call form writes it so).
    if ((a.k === "num" || a.k === "lit") && (b.k === "num" || b.k === "lit")) {
      const int = a.k === "num" && b.k === "num" && a.int && b.int;
      if (a.k === "num" && b.k === "num") return num(foldNum(op, a.v, b.v, int), int);
      const xs = a.k === "lit" ? a.v : null, ys = b.k === "lit" ? b.v : null;
      const n = (xs ?? ys)!.length;
      return { k: "lit", v: Array.from({ length: n }, (_, i) => foldNum(op, xs ? xs[i]! : (a as { v: number }).v, ys ? ys[i]! : (b as { v: number }).v, false)) };
    }
    const x = callArg(a)!, y = callArg(b)!;
    switch (op) {
      case "+": return ex(X.add(x as never, y as never));
      case "-": return ex(X.sub(x as never, y as never));
      case "*": return ex(X.mul(x as never, y as never));
      case "/": return ex(X.div(x as never, y as never));
      case "%": return ex(X.rem(x as never, y as never));
    }
    return this.fail(at, `the operator ${op}`, "+ - * / %", "use one of those");
  }

  /** Matrices, as their columns: m * v = sum of column j times v[j]; v * m = (dot(v, column j)); m * m column by column. */
  matArith(op: string, a: Value, b: Value, at: At): Value {
    if (a.k === "mat" && b.k === "mat") {
      if (op === "+" || op === "-") return { k: "mat", cols: a.cols.map((c, i) => this.arith(op, c, b.cols[i]!, at)) };
      if (op === "*") return { k: "mat", cols: b.cols.map((c) => this.matArith("*", a, c, at)) };
    }
    if (op === "*" && a.k === "mat" && numTy(b) !== undefined && numTy(b) !== "f32") {
      if (Number(numTy(b)!.slice(3)) !== a.cols.length) this.fail(at, `${typeName(a)} * ${typeName(b)}`, `a vec${a.cols.length}f`, "match the matrix's size");
      return a.cols.map((c, j) => this.arith("*", c, this.component(b, j, at), at)).reduce((s, t) => this.arith("+", s, t, at));
    }
    if (op === "*" && b.k === "mat" && numTy(a) !== undefined && numTy(a) !== "f32") {
      return this.construct(`vec${b.cols.length}` as X.VecTy, b.cols.map((c) => this.dot(a, c, at)), at);
    }
    if (op === "*" && a.k === "mat" && numTy(b) === "f32") return { k: "mat", cols: a.cols.map((c) => this.arith("*", c, b, at)) };
    if (op === "*" && b.k === "mat" && numTy(a) === "f32") return { k: "mat", cols: b.cols.map((c) => this.arith("*", a, c, at)) };
    return this.fail(at, `${typeName(a)} ${op} ${typeName(b)}`, "matrix times vector, vector times matrix, matrix times matrix or scalar", "check the operands");
  }

  dot(a: Value, b: Value, at: At): Value {
    const ta = numTy(a), tb = numTy(b);
    if (ta === undefined || ta === "f32" || ta !== tb) this.fail(at, `dot of ${typeName(a)} and ${typeName(b)}`, "two vectors of one size", "dot(a, b) with a and b the same vector type");
    return ex(X.dot(callArg(a)! as never, callArg(b)! as never));
  }

  // ---- access ----

  member(v: Value, name: string, at: At): Value {
    switch (v.k) {
      case "struct": {
        const f = v.fields.get(name);
        if (f === undefined) this.fail(at, `.${name} of ${v.name}`, `one of its fields (${[...v.fields.keys()].join(", ")})`, closest(name, [...v.fields.keys()]) !== undefined ? `did you mean .${closest(name, [...v.fields.keys()])}?` : "use a field the struct has");
        return f;
      }
      case "open": {
        if (!(name in v.obj)) this.fail(at, `.${name} of ${v.name}`, `one of its fields (${Object.keys(v.obj).join(", ")})`, "use a field the record has");
        return fromTs(v.obj[name], `${v.name}.${name}`, (g, e, f) => this.fail(at, g, e, f));
      }
      case "mat": return this.fail(at, `.${name} of a matrix`, "a column by index: m[0]", "index the column");
      default: {
        const ty = numTy(v);
        if (ty === undefined || ty === "f32") this.fail(at, `.${name} of ${typeName(v)}`, "a vector's components or a struct's field", `${typeName(v)} has no .${name}`);
        const sel = this.swizzleOf(name, ty, at);
        if (v.k === "lit") { const out = [...sel].map((c) => v.v["xyzw".indexOf(c)]!); return out.length === 1 ? num(out[0]!) : { k: "lit", v: out }; }
        return ex(X.swizzle((v as { e: E }).e as X.Expr<X.VecTy>, sel));
      }
    }
  }

  /** `.rgb` is `.xyz`; every letter names a component the vector has. */
  swizzleOf(name: string, ty: X.NumTy, at: At): string {
    const n = Number(ty.slice(3));
    const sel = /^[rgba]+$/.test(name) ? [...name].map((c) => "xyzw"["rgba".indexOf(c)]).join("") : name;
    if (!/^[xyzw]{1,4}$/.test(sel) || [...sel].some((c) => "xyzw".indexOf(c) >= n)) this.fail(at, `.${name} of a ${ty}`, `components of ${"xyzw".slice(0, n)} (or ${"rgba".slice(0, n)})`, `a ${ty} has ${n} components`);
    return sel;
  }

  component(v: Value, i: number, at: At): Value {
    if (v.k === "lit") return num(v.v[i]!);
    if (v.k === "num") return v;
    if (v.k === "expr" && v.e.ty === "f32") return v;
    return this.member(v, "xyzw"[i]!, at);
  }

  /** The whole number an index value is, when it is a constant. */
  whole(i: Value, at: At): number | undefined {
    if ((i.k === "num" && Number.isInteger(i.v)) || (i.k === "int" && typeof i.v === "number")) return i.v as number;
    if (i.k === "int" || (i.k === "expr" && i.e.ty === "f32")) return undefined;
    return this.fail(at, `an index of ${typeName(i)}`, "an integer", "index with an integer (a loop counter or a literal)");
  }

  index(v: Value, i: Value, at: At): Value {
    const k = this.whole(i, at);
    const length = v.k === "array" ? v.elems.length : v.k === "mat" ? v.cols.length : numTy(v) !== undefined && numTy(v) !== "f32" ? Number(numTy(v)!.slice(3)) : this.fail(at, `an index into ${typeName(v)}`, "an array, a matrix or a vector", "index an array");
    const items = (): Value[] => (v.k === "array" ? [...v.elems] : v.k === "mat" ? [...v.cols] : Array.from({ length }, (_, j) => this.component(v, j, at)));
    if (k !== undefined) {
      if (k < 0 || k >= length) this.fail(at, `index ${k}`, `0 to ${length - 1}`, "index inside the array");
      return items()[k]!;
    }
    // An index that is an expression: one select per item after the first (src/expr.ts `pick`).
    const xs = items();
    const index = i.k === "int" ? i.v : (i as { e: E }).e;
    const args = xs.map((x) => callArg(x));
    if (args.some((x) => x === undefined)) this.fail(at, "an expression index into an array of structs or arrays", "an array of numbers or vectors", "index a constant, or split the array by field");
    return ex(X.pick(index as X.Expr<"f32">, args.map((x) => X.lift(x as X.In)) as X.Expr<X.NumTy>[]));
  }

  // ---- constructors ----

  /** vecN, mat, array and f32 constructors; undefined if `name` is none. */
  ctor(name: string, type: TypeRef | undefined, args: readonly Value[], at: At): Value | undefined {
    const vec = /^vec([234])(f?)$/.exec(name);
    if (vec) {
      if (type !== undefined && type.args[0]?.name !== "f32") this.fail(at, `${name}<${type.args[0]?.name}>`, `${name}<f32>`, "vectors are f32 in the subset");
      const ty = `vec${vec[1]}` as X.VecTy;
      if (args.length === 0) return { k: "lit", v: Array(Number(vec[1])).fill(0) };
      return this.construct(ty, args, at);
    }
    const mat = /^mat([23])x([23])(f?)$/.exec(name);
    if (mat) {
      if (mat[1] !== mat[2]) this.fail(at, name, "mat2x2f or mat3x3f", "square matrices only");
      const n = Number(mat[1]);
      if (args.length === n) return { k: "mat", cols: args.map((c) => { if (numTy(c) !== `vec${n}`) this.fail(at, `a column of ${typeName(c)}`, `vec${n}f`, "give columns"); return c; }) };
      if (args.length === n * n) return { k: "mat", cols: Array.from({ length: n }, (_, j) => this.construct(`vec${n}` as X.VecTy, args.slice(j * n, j * n + n), at)) };
      return this.fail(at, `${args.length} arguments to ${name}`, `${n} columns or ${n * n} numbers (column by column)`, `${name}(c0, c1${n === 3 ? ", c2" : ""})`);
    }
    if (name === "array") {
      if (args.length === 0) this.fail(at, "an empty array", "at least one element", "give the elements");
      if (type?.count !== undefined && type.count !== args.length) this.fail(at, `${args.length} elements`, `${type.count}`, "match the declared count");
      return { k: "array", elems: [...args] };
    }
    if (name === "f32") {
      if (args.length !== 1) this.fail(at, `${args.length} arguments to f32`, "one", "f32(x)");
      const v = args[0]!;
      if (v.k === "num") return num(v.v);
      if (v.k === "int") return typeof v.v === "number" ? num(v.v) : ex(v.v);
      if (v.k === "expr" && v.e.ty === "f32") return v;
      if (v.k === "bool") return num(v.b ? 1 : 0);
      if (v.k === "expr" && v.e.ty === "bool") return ex(X.select(v.e as BoolE, 1, 0));
      return this.fail(at, `f32 of ${typeName(v)}`, "a number, an integer or a bool", "take a component first");
    }
    if (name === "bool") {
      const v = args[0];
      if (args.length === 1 && v !== undefined && asB(v) !== undefined) return v;
      if (args.length === 1 && v !== undefined && numTy(v) === "f32") return ex(X.ne(callArg(v) as X.In<"f32">, 0));
      return this.fail(at, "bool of this", "bool(x) of an f32 or a bool", "compare instead: x != 0.0");
    }
    return undefined;
  }

  /** A vector of type `ty` from parts (numbers, vectors, a single scalar splatted): a constant when every part is one. */
  construct(ty: X.VecTy, parts: readonly Value[], at: At): Value {
    const n = Number(ty.slice(3));
    let total = 0;
    for (const p of parts) {
      const t = p.k === "int" ? this.intMix(num(0), at) : numTy(p);
      if (t === undefined) this.fail(at, `${typeName(p)} in ${ty}f(...)`, "numbers and vectors", "build vectors from numbers");
      total += t === "f32" ? 1 : Number(t.slice(3));
    }
    if (total !== n && !(parts.length === 1 && numTy(parts[0]!) === "f32")) this.fail(at, `${total} components in ${ty}f(...)`, `${n} (or one number for all of them)`, `give exactly ${n}`);
    if (parts.every((p) => p.k === "num" || p.k === "lit")) {
      const flat = parts.flatMap((p) => (p.k === "num" ? [p.v] : (p as { v: readonly number[] }).v));
      return { k: "lit", v: flat.length === 1 ? Array(n).fill(flat[0]) : flat };
    }
    const args = parts.map((p) => callArg(p)!);
    return ex(({ vec2: X.vec2, vec3: X.vec3, vec4: X.vec4 } as const)[ty](...(args as X.Num[])));
  }

  structOf(name: string, fields: readonly Param[], args: readonly Value[], at: At): Value {
    if (args.length !== fields.length) this.fail(at, `${args.length} values for ${name}`, `${fields.length} (${fields.map((f) => f.name).join(", ")})`, `${name}(${fields.map((f) => f.name).join(", ")})`);
    return { k: "struct", name, fields: new Map(fields.map((f, i) => [f.name, this.conform(args[i]!, f.type, at, `${name}.${f.name}`)])) };
  }

  // ---- types ----

  /** `v` as a value of the declared type `t`: an abstract number becomes an f32, a record a struct; anything else must already match. */
  conform(v: Value, t: TypeRef, at: At, what: string, caller?: string): Value {
    const bad = (expected: string, fix: string): never => this.fail(at, `${what} given ${typeName(v)}`, expected, fix, caller);
    const name = t.name;
    if (name === "f32") {
      if (v.k === "num") return num(v.v);
      if (v.k === "expr" && v.e.ty === "f32") return v;
      if (v.k === "int") return bad("f32", "convert: f32(i)");
      return bad("f32", "pass a number");
    }
    const vec = /^vec([234])f?$/.exec(name);
    if (vec) {
      const want = `vec${vec[1]}`;
      if (numTy(v) === want) return v;
      return bad(`${want}f`, numTy(v) === "f32" ? `make a vector: ${want}f(x)` : `pass a ${want}f`);
    }
    if (name === "bool") return asB(v) !== undefined ? v : bad("bool", "pass a comparison");
    if (name === "i32") {
      if (v.k === "int") return v;
      if (v.k === "num" && v.int) return { k: "int", v: v.v };
      return bad("i32", "pass an integer");
    }
    const mat = /^mat([23])x\1f?$/.exec(name);
    if (mat) return v.k === "mat" && v.cols.length === Number(mat[1]) ? v : bad(name, "pass a matrix");
    if (name === "array") {
      if (v.k !== "array") return bad("an array", "pass an array");
      if (t.count !== undefined && v.elems.length !== t.count) return bad(`array of ${t.count}`, `give ${t.count} elements`);
      return t.args[0] === undefined ? v : { k: "array", elems: v.elems.map((e, i) => this.conform(e, t.args[0]!, at, `${what}[${i}]`, caller)) };
    }
    const st = this.m.structs.get(name) ?? PRELUDE_STRUCTS.get(name);
    if (st !== undefined) {
      if (v.k === "struct" && v.name === name) return v;
      // A record from TypeScript or an open record: read the struct's fields from it, by name.
      const get = (f: string): Value | undefined =>
        v.k === "struct" ? v.fields.get(f) : v.k === "open" ? (f in v.obj ? fromTs(v.obj[f], `${what}.${f}`, (g, e, x) => this.fail(at, g, e, x, caller)) : undefined) : undefined;
      if (v.k !== "struct" && v.k !== "open") return bad(name, `pass a ${name}`);
      if ("open" in st && st.open) return v.k === "open" ? { ...v, name } : v;
      const fields = new Map<string, Value>();
      for (const f of st.fields) {
        const fv = get(f.name);
        if (fv === undefined) return bad(`${name} (${st.fields.map((x) => `${x.name}: ${x.type.name}`).join(", ")})`, `give it a ${f.name}`);
        fields.set(f.name, this.conform(fv, f.type, at, `${what}.${f.name}`, caller));
      }
      return { k: "struct", name, fields };
    }
    return bad(`a type of the subset (${name} is not one)`, "use f32, vec2f..vec4f, bool, a matrix, an array, or a struct of these");
  }
}

// ---- prelude structs ------------------------------------------------------------------------------------------------

/** Structs a slot passes. `open`: the record is passed through as it is (Ray carries a function, `width`). */
export type PreludeStruct = { readonly name: string; readonly fields: readonly Param[]; readonly open: boolean };
const at0: At = { line: 0, col: 0 };
const ty = (name: string): TypeRef => ({ name, args: [], count: undefined, at: at0 });
export const PRELUDE_STRUCTS: ReadonlyMap<string, PreludeStruct> = new Map([
  ["PaintAt", { name: "PaintAt", open: false, fields: [{ name: "world", type: ty("vec3f"), at: at0 }, { name: "normal", type: ty("vec3f"), at: at0 }, { name: "copy", type: ty("f32"), at: at0 }] }],
  ["Ray", { name: "Ray", open: true, fields: [{ name: "o", type: ty("vec3f"), at: at0 }, { name: "d", type: ty("vec3f"), at: at0 }, { name: "t0", type: ty("f32"), at: at0 }, { name: "t1", type: ty("f32"), at: at0 }] }],
]);

// ---- builtins -------------------------------------------------------------------------------------------------------

type Builtin = (l: Lowerer, args: readonly Value[], at: At) => Value;
const arity = (l: Lowerer, name: string, args: readonly Value[], n: number, at: At): void => {
  if (args.length !== n) l.fail(at, `${args.length} argument${args.length === 1 ? "" : "s"} to ${name}`, `${n}`, `${name}(${["a", "b", "c"].slice(0, n).join(", ")})`);
};
/** Arguments as the call form takes them (numbers, arrays, expressions), each checked to be numeric. */
const nums = (l: Lowerer, name: string, args: readonly Value[], at: At): X.Num[] => args.map((a, i) => {
  const x = numTy(a) === undefined ? undefined : callArg(a);
  if (x === undefined) {
    if (a.k === "int") l.intMix(num(0), at);
    l.fail(at, `${name}'s argument ${i + 1} of ${typeName(a)}`, "a number or a vector", "pass numbers");
  }
  return x as X.Num;
});
const unary = (name: string, f: (a: never) => E, vecOnly = false): [string, Builtin] => [name, (l, args, at) => {
  arity(l, name, args, 1, at);
  const [a] = nums(l, name, args, at);
  if (vecOnly && numTy(args[0]!) === "f32") l.fail(at, `${name} of an f32`, "a vector", `${name} takes a vector`);
  return ex(f(a as never));
}];
const nary = (name: string, n: number, f: (...a: never[]) => E): [string, Builtin] => [name, (l, args, at) => {
  arity(l, name, args, n, at);
  const xs = nums(l, name, args, at);
  const tys = args.map((a) => numTy(a)!).filter((t) => t !== "f32");
  if (new Set(tys).size > 1) l.fail(at, `${name} of ${args.map(typeName).join(", ")}`, "arguments of one vector size (or scalars)", "match the sizes");
  return ex(f(...(xs as never[])));
}];
const bvecOf = (l: Lowerer, name: string, v: Value, at: At): readonly B[] => {
  if (v.k === "bvec") return v.parts;
  const b = asB(v);
  if (b !== undefined) return [b];
  return l.fail(at, `${name} of ${typeName(v)}`, "a bool or a vector comparison", `${name}(a < b)`);
};
const DEG = Math.PI / 180;

/** The WGSL builtins graph text lowers; `loft context` prints their names from here, so its list cannot drift. */
export const BUILTINS = new Map<string, Builtin>([
  unary("abs", X.abs), unary("sign", X.sign), unary("floor", X.floor), unary("ceil", X.ceil), unary("fract", X.fract), unary("trunc", X.trunc),
  // WGSL's round is half to even; Loft's call-form `round` is floor(x + 0.5). Text means WGSL.
  unary("round", X.roundEven), unary("sqrt", X.sqrt), unary("inverseSqrt", X.inverseSqrt), unary("exp", X.exp), unary("exp2", X.exp2),
  unary("log", X.log), unary("log2", X.log2), unary("sin", X.sin), unary("cos", X.cos), unary("tan", X.tan), unary("asin", X.asin),
  unary("acos", X.acos), unary("atan", X.atan), unary("sinh", X.sinh), unary("cosh", X.cosh), unary("tanh", X.tanh),
  unary("saturate", X.saturate), unary("normalize", X.normalize, true),
  nary("min", 2, X.min), nary("max", 2, X.max), nary("pow", 2, X.pow), nary("atan2", 2, X.atan2), nary("step", 2, X.step),
  nary("mix", 3, X.mix), nary("clamp", 3, X.clamp), nary("smoothstep", 3, X.smoothstep),
  ["length", (l, args, at) => { arity(l, "length", args, 1, at); return ex(X.length(nums(l, "length", args, at)[0] as never)); }],
  ["distance", (l, args, at) => { arity(l, "distance", args, 2, at); const [a, b] = nums(l, "distance", args, at); if (numTy(args[0]!) !== numTy(args[1]!)) l.fail(at, `distance of ${args.map(typeName).join(" and ")}`, "two values of one type", "match the types"); return ex(X.distance(a as never, b as never)); }],
  ["dot", (l, args, at) => { arity(l, "dot", args, 2, at); return l.dot(args[0]!, args[1]!, at); }],
  ["cross", (l, args, at) => { arity(l, "cross", args, 2, at); if (numTy(args[0]!) !== "vec3" || numTy(args[1]!) !== "vec3") l.fail(at, `cross of ${args.map(typeName).join(" and ")}`, "two vec3f", "cross(a, b) of vec3s"); const [a, b] = nums(l, "cross", args, at); return ex(X.cross(a as never, b as never)); }],
  // fma lowers to a multiply and an add in both backends (WGSL's fused rounding is not promised).
  ["fma", (l, args, at) => { arity(l, "fma", args, 3, at); return l.arith("+", l.arith("*", args[0]!, args[1]!, at), args[2]!, at); }],
  ["radians", (l, args, at) => { arity(l, "radians", args, 1, at); return l.arith("*", args[0]!, num(DEG), at); }],
  ["degrees", (l, args, at) => { arity(l, "degrees", args, 1, at); return l.arith("*", args[0]!, num(1 / DEG), at); }],
  // reflect(i, n) = i - 2 dot(n, i) n.
  ["reflect", (l, args, at) => { arity(l, "reflect", args, 2, at); const [i, n] = args as [Value, Value]; return l.arith("-", i, l.arith("*", l.arith("*", num(2), l.dot(n, i, at), at), n, at), at); }],
  // faceForward(n, i, nref) = n if dot(nref, i) < 0, else -n.
  ["faceForward", (l, args, at) => { arity(l, "faceForward", args, 3, at); const [n, i, r] = args as [Value, Value, Value]; return l.merge(asB(l.compare("<", l.dot(r, i, at), num(0), at))!, n, l.arith("*", n, num(-1), at)); }],
  // refract(i, n, eta): k = 1 - eta^2 (1 - dot(n, i)^2); 0 where k < 0, else eta i - (eta dot(n, i) + sqrt(k)) n.
  ["refract", (l, args, at) => {
    arity(l, "refract", args, 3, at);
    const [i, n, eta] = args as [Value, Value, Value];
    const d = l.dot(n, i, at);
    const k = l.arith("-", num(1), l.arith("*", l.arith("*", eta, eta, at), l.arith("-", num(1), l.arith("*", d, d, at), at), at), at);
    const root = ex(X.sqrt(X.max(callArg(k) as never, 0)));
    const out = l.arith("-", l.arith("*", eta, i, at), l.arith("*", l.arith("+", l.arith("*", eta, d, at), root, at), n, at), at);
    return l.merge(asB(l.compare("<", k, num(0), at))!, { k: "lit", v: Array(Number(numTy(i)!.slice(3))).fill(0) }, out);
  }],
  ["select", (l, args, at) => {
    arity(l, "select", args, 3, at);
    const [no, yes, c] = args as [Value, Value, Value];
    if (c.k === "bvec") {
      const n = c.parts.length;
      return l.construct(`vec${n}` as X.VecTy, c.parts.map((p, j) => l.merge(p, l.component(yes, j, at), l.component(no, j, at))), at);
    }
    const cb = asB(c);
    if (cb === undefined) return l.fail(at, `select's condition of ${typeName(c)}`, "a bool (select(no, yes, condition), WGSL's order)", "select(f, t, x > 0.0)");
    // A bool merge keeps bools; numbers become one select node, as the call form's select(cond, yes, no).
    return l.merge(cb, yes, no);
  }],
  ["all", (l, args, at) => { arity(l, "all", args, 1, at); return fromB(bvecOf(l, "all", args[0]!, at).reduce((a, b) => (typeof a === "boolean" || typeof b === "boolean" ? boolAnd(a, b) : X.and(a, b)))); }],
  ["any", (l, args, at) => { arity(l, "any", args, 1, at); return fromB(bvecOf(l, "any", args[0]!, at).reduce((a, b) => (typeof a === "boolean" || typeof b === "boolean" ? boolOr(a, b) : X.or(a, b)))); }],
]);
const BUILTIN_NAMES = [...BUILTINS.keys(), "vec2f", "vec3f", "vec4f", "mat2x2f", "mat3x3f", "array", "f32", "gradient", "time"];

// ---- helpers --------------------------------------------------------------------------------------------------------

/** A value as the call form's constructors take it: a number, a plain array or an expression (undefined for others). */
export function callArg(v: Value): X.Num | X.Expr<"bool"> | undefined {
  switch (v.k) {
    case "num": return v.v;
    case "lit": return v.v;
    case "expr": return v.e as NumE;
    case "int": return v.v;
    case "bool": return X.lift<"bool">(v.b);
    default: return undefined;
  }
}

function select(c: BoolE, a: X.Num | X.Expr<"bool">, b: X.Num | X.Expr<"bool">): E {
  // select(!x, a, b) is select(x, b, a): a mask by "not returned" reads as the branch it guards.
  if (c.node.op === "call" && c.node.fn === "not") return X.select(c.node.args[0] as BoolE, b as never, a as never);
  return X.select(c, a as never, b as never);
}
const boolNot = (a: B): B => {
  if (typeof a === "boolean") return !a;
  return a.node.op === "call" && a.node.fn === "not" ? (a.node.args[0] as BoolE) : X.not(a);
};
/** Whether one is `not` of the other. */
const complements = (a: BoolE, b: BoolE): boolean => (a.node.op === "call" && a.node.fn === "not" && a.node.args[0] === b) || (b.node.op === "call" && b.node.fn === "not" && b.node.args[0] === a);
const boolOr = (a: B, b: B): B => {
  const ka = constBool(a), kb = constBool(b);
  if (ka === true || kb === true) return true;
  if (ka === undefined && kb === undefined && complements(a as BoolE, b as BoolE)) return true;
  if (ka === false) return b;
  if (kb === false) return a;
  return a === b ? a : X.or(a as BoolE, b as BoolE);
};
const boolAnd = (a: B, b: B): B => {
  const ka = constBool(a), kb = constBool(b);
  if (ka === false || kb === false) return false;
  if (ka === undefined && kb === undefined && complements(a as BoolE, b as BoolE)) return false;
  if (ka === true) return b;
  if (kb === true) return a;
  return a === b ? a : X.and(a as BoolE, b as BoolE);
};
/** `yes` where `c` holds, else `no`, for bools (Loft's select is numeric). */
function boolSelect(c: BoolE, yes: B, no: B): B {
  if (yes === no) return yes;
  const ky = constBool(yes), kn = constBool(no);
  if (ky === true && kn === false) return c;
  if (ky === false && kn === true) return boolNot(c);
  if (ky === true) return boolOr(c, no);
  if (ky === false) return boolAnd(boolNot(c), no);
  if (kn === true) return boolOr(boolNot(c), yes);
  if (kn === false) return boolAnd(c, yes);
  return boolOr(boolAnd(c, yes), boolAnd(boolNot(c), no));
}

function sameValue(a: Value, b: Value): boolean {
  if (a === b) return true;
  if (a.k !== b.k) return false;
  switch (a.k) {
    case "num": return a.v === (b as typeof a).v;
    case "lit": return a.v.length === (b as typeof a).v.length && a.v.every((x, i) => x === (b as typeof a).v[i]);
    case "expr": return a.e === (b as typeof a).e;
    case "bool": return a.b === (b as typeof a).b;
    case "int": return a.v === (b as typeof a).v;
    case "struct": return [...a.fields].every(([k, v]) => sameValue(v, (b as typeof a).fields.get(k)!));
    case "array": return a.elems.every((v, i) => sameValue(v, (b as typeof a).elems[i]!));
    case "mat": return a.cols.every((v, i) => sameValue(v, (b as typeof a).cols[i]!));
    case "bvec": return a.parts.every((p, i) => p === (b as typeof a).parts[i]);
    default: return false;
  }
}

function cmpNum(op: string, x: number, y: number): boolean {
  switch (op) { case "<": return x < y; case ">": return x > y; case "<=": return x <= y; case ">=": return x >= y; case "==": return x === y; default: return x !== y; }
}
/** A constant operation; between two abstract integers, / and % are WGSL's integer ones. */
function foldNum(op: string, x: number, y: number, int: boolean): number {
  switch (op) {
    case "+": return x + y;
    case "-": return x - y;
    case "*": return x * y;
    case "/": return int ? Math.trunc(x / y) : x / y;
    default: return x % y;
  }
}

/** Names of outer variables a loop body assigns (not those it declares itself). */
function assignedNames(body: readonly Stmt[], declared: ReadonlySet<string>): Set<string> {
  const out = new Set<string>();
  const walk = (list: readonly Stmt[], decl: Set<string>): void => {
    const local = new Set(decl);
    for (const s of list) {
      switch (s.k) {
        case "let": case "const": case "var": local.add(s.name); break;
        case "assign": case "step": if (!local.has(s.place.name)) out.add(s.place.name); break;
        case "if": walk(s.then, local); if (s.else) walk(s.else, local); break;
        case "block": walk(s.body, local); break;
        case "for": {
          const inner = new Set(local);
          if (s.init?.k === "var") inner.add(s.init.name);
          walk(s.body, inner);
          break;
        }
        default: break;
      }
    }
  };
  walk(body, new Set(declared));
  // The counter is declared by the loop, so it is never "outer"; a write to it is still found by name by the caller.
  for (const s of flat(body)) if ((s.k === "assign" || s.k === "step") && declared.has(s.place.name)) out.add(s.place.name);
  return out;
}
function flat(body: readonly Stmt[]): Stmt[] {
  const out: Stmt[] = [];
  const walk = (list: readonly Stmt[]): void => {
    for (const s of list) {
      out.push(s);
      if (s.k === "if") { walk(s.then); if (s.else) walk(s.else); }
      if (s.k === "block") walk(s.body);
      if (s.k === "for") walk(s.body);
    }
  };
  walk(body);
  return out;
}
/** WGSL's "a function returns on every path", from the statements: a return, or an if whose branches both return. */
function alwaysReturns(body: readonly Stmt[]): boolean {
  return body.some((s) => s.k === "return" || (s.k === "if" && s.else !== undefined && alwaysReturns(s.then) && alwaysReturns(s.else)) || (s.k === "block" && alwaysReturns(s.body)));
}
const containsReturn = (body: readonly Stmt[]): boolean => flat(body).some((s) => s.k === "return");
/** A break of this loop (not of a loop nested in it). */
function containsBreak(body: readonly Stmt[]): boolean {
  for (const s of body) {
    if (s.k === "break") return true;
    if (s.k === "if" && (containsBreak(s.then) || (s.else !== undefined && containsBreak(s.else)))) return true;
    if (s.k === "block" && containsBreak(s.body)) return true;
  }
  return false;
}

/** How a value is carried through a loop: as numeric components, rebuilt with the same shape. */
type Layout = { readonly size: number; flatten(v: Value): NumE[]; rebuild(xs: readonly NumE[]): Value };
function layoutOf(v: Value, l: Lowerer, at: At, name: string): Layout {
  switch (v.k) {
    case "num": case "lit": return { size: 1, flatten: (x) => [X.lift(callArg(x) as X.In) as NumE], rebuild: (xs) => ex(xs[0]!) };
    case "int": return { size: 1, flatten: (x) => [X.lift((x as { v: number | E }).v as X.In) as NumE], rebuild: (xs) => ({ k: "int", v: xs[0] as X.Expr<"f32"> }) };
    case "expr": case "bool":
      if (v.k === "expr" && v.e.ty !== "bool") return { size: 1, flatten: (x) => [X.lift(callArg(x) as X.In) as NumE], rebuild: (xs) => ex(xs[0]!) };
      // A bool is carried as 1 or 0.
      return { size: 1, flatten: (x) => { const b = asB(x)!; return [typeof b === "boolean" ? X.f(b ? 1 : 0) : X.select(b, 1, 0)]; }, rebuild: (xs) => ex(X.ne(xs[0] as X.Expr<"f32">, 0)) };
    case "struct": {
      const parts = [...v.fields].map(([k, f]) => [k, layoutOf(f, l, at, `${name}.${k}`)] as const);
      return {
        size: parts.reduce((s, [, p]) => s + p.size, 0),
        flatten: (x) => parts.flatMap(([k, p]) => p.flatten((x as { fields: ReadonlyMap<string, Value> }).fields.get(k)!)),
        rebuild: (xs) => { let i = 0; return { k: "struct", name: v.name, fields: new Map(parts.map(([k, p]) => { const r = p.rebuild(xs.slice(i, i + p.size)); i += p.size; return [k, r]; })) }; },
      };
    }
    case "array": case "mat": {
      const items = v.k === "array" ? v.elems : v.cols;
      const parts = items.map((e, i) => layoutOf(e, l, at, `${name}[${i}]`));
      const list = (x: Value): readonly Value[] => (x.k === "array" ? x.elems : (x as { cols: readonly Value[] }).cols);
      return {
        size: parts.reduce((s, p) => s + p.size, 0),
        flatten: (x) => parts.flatMap((p, i) => p.flatten(list(x)[i]!)),
        rebuild: (xs) => { let i = 0; const made = parts.map((p) => { const r = p.rebuild(xs.slice(i, i + p.size)); i += p.size; return r; }); return v.k === "array" ? { k: "array", elems: made } : { k: "mat", cols: made }; },
      };
    }
    default: return l.fail(at, `${name}, ${typeName(v)}, assigned in a loop`, "numbers, vectors, bools, or structs and arrays of them", "keep it outside the loop");
  }
}

/** The zero of a declared type, for a var a loop assigns before anything else does. */
function zeroOf(t: TypeRef, l: Lowerer, at: At): Value {
  if (t.name === "f32") return num(0);
  const vec = /^vec([234])f?$/.exec(t.name);
  if (vec) return { k: "lit", v: Array(Number(vec[1])).fill(0) };
  if (t.name === "bool") return fromB(false);
  if (t.name === "i32") return { k: "int", v: 0 };
  const st = l.m.structs.get(t.name);
  if (st) return { k: "struct", name: st.name, fields: new Map(st.fields.map((f) => [f.name, zeroOf(f.type, l, at)])) };
  if (t.name === "array" && t.count !== undefined && t.args[0]) return { k: "array", elems: Array.from({ length: t.count }, () => zeroOf(t.args[0]!, l, at)) };
  return l.fail(at, `a var of type ${t.name} with no value`, "a value", "give it one");
}

/** The nearest of `names` to `name` by edit distance, if near enough to be a typo. */
export function closest(name: string, names: readonly string[]): string | undefined {
  let best: string | undefined, bestD = Infinity;
  for (const n of names) {
    const d = editDistance(name, n);
    if (d < bestD) { best = n; bestD = d; }
  }
  return bestD <= Math.max(1, Math.floor(name.length / 3)) ? best : undefined;
}
function editDistance(a: string, b: string): number {
  const d = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    let prev = d[0]!;
    d[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const t = d[j]!;
      d[j] = Math.min(d[j]! + 1, d[j - 1]! + 1, prev + (a[i - 1] === b[j - 1] ? 0 : 1));
      prev = t;
    }
  }
  return d[b.length]!;
}

// ---- TypeScript values in and out -----------------------------------------------------------------------------------

type Fail = (given: string, expected: string, fix: string) => never;

/** A TypeScript value (a hole, an argument, a field of a record) as a value in the text. */
export function fromTs(x: unknown, what: string, fail: Fail): Value {
  if (typeof x === "number") return num(x);
  if (typeof x === "boolean") return fromB(x);
  if (x instanceof X.Expr) return ex(x);
  if (typeof x === "string") return fail(`${what}, a string`, "a value (a number, an array, an expression, a function or a record)", "the template splices values, never text: write the text itself, or pass the value");
  if (typeof x === "function") {
    const f = x as (...a: unknown[]) => unknown;
    return { k: "fn", name: f.name || what, call: (args) => fromTs(f(...args.map((a) => toArg(a, what, fail))), `${what}(...)`, fail) };
  }
  if (Array.isArray(x)) {
    if (x.length >= 2 && x.length <= 4 && x.every((c) => typeof c === "number")) return { k: "lit", v: x as number[] };
    return { k: "array", elems: x.map((e, i) => fromTs(e, `${what}[${i}]`, fail)) };
  }
  if (typeof x === "object" && x !== null) return { k: "open", name: what, obj: x as Record<string, unknown> };
  return fail(`${what}, ${x === undefined ? "undefined" : String(x)}`, "a value", "pass a number, an array, an expression, a function or a record");
}

/**
 * A value as an argument to a TypeScript function of expressions: numbers and constant vectors stay plain (as a call
 * form passes them, so the function lifts them where it uses them and the nodes come in the call form's order).
 */
function toArg(v: Value, what: string, fail: Fail): unknown {
  switch (v.k) {
    case "num": return v.v;
    case "lit": return v.v;
    case "bool": return v.b;
    case "int": return v.v;
    default: return toTs(v, what, fail);
  }
}

/** A value in the text as TypeScript sees it: expressions (numbers lifted), records for structs, functions for functions. */
export function toTs(v: Value, what: string, fail: Fail): unknown {
  switch (v.k) {
    case "num": return X.f(v.v);
    case "lit": return X.lift(v.v as readonly number[]);
    case "expr": return v.e;
    case "bool": return X.lift<"bool">(v.b);
    case "int": return X.lift<"f32">(v.v);
    case "struct": return Object.fromEntries([...v.fields].map(([k, f]) => [k, toTs(f, `${what}.${k}`, fail)]));
    case "open": return v.obj;
    case "array": return v.elems.map((e, i) => toTs(e, `${what}[${i}]`, fail));
    case "mat": return v.cols.map((c, i) => toTs(c, `${what}[${i}]`, fail));
    case "fn": return (...args: unknown[]) => toTs(v.call(args.map((a, i) => fromTs(a, `${what} argument ${i + 1}`, fail)), at0), what, fail);
    case "bvec": return fail(`${what}, a vector of bools`, "a value TypeScript can hold", "join it with all(...) or any(...)");
  }
}

/** The first frame outside Loft's source: the TypeScript line that called into the text. */
export const callerSite = (): string | undefined => siteOf(new Error().stack);
