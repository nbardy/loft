// WGSL text to a module of items. Each function is classified once, here, at the boundary: a graph function (the
// subset, parsed to statements that src/text/lower.ts lowers into the expression graph per call), a raw function
// (outside the subset, with the reason and the line that put it there; emitted verbatim as a GPU-only library
// function), or an entry point (`@vertex`, `@fragment`, `@compute`: raw drawables and compute passes read those).

import { HOLE_PREFIX, tokenize, TokenError, type Tok } from "./tokens.ts";

// ---- the tree ---------------------------------------------------------------------------------------------------------

/** Where a construct is, in the module text (0-based line and column). */
export type At = { readonly line: number; readonly col: number };

/** A type as written: `f32`, `vec3<f32>` (args [f32]), `array<f32, 4>` (args [f32], count 4), a struct's name. */
export type TypeRef = { readonly name: string; readonly args: readonly TypeRef[]; readonly count: number | undefined; readonly at: At };

export type Expr =
  | { readonly k: "num"; readonly v: number; readonly int: boolean; readonly text: string; readonly at: At }
  | { readonly k: "bool"; readonly v: boolean; readonly at: At }
  | { readonly k: "ident"; readonly name: string; readonly at: At }
  | { readonly k: "unary"; readonly op: "-" | "!"; readonly a: Expr; readonly at: At }
  | { readonly k: "binary"; readonly op: string; readonly a: Expr; readonly b: Expr; readonly at: At }
  /** `callee(args)`; `type` is set for a constructor written with template arguments (`vec3<f32>(...)`, `array<f32, 3>(...)`). */
  | { readonly k: "call"; readonly callee: Expr; readonly type: TypeRef | undefined; readonly args: readonly Expr[]; readonly at: At }
  | { readonly k: "member"; readonly a: Expr; readonly name: string; readonly at: At }
  | { readonly k: "index"; readonly a: Expr; readonly i: Expr; readonly at: At };

/** What an assignment writes: a name, then fields, swizzles and indices. */
export type Place = { readonly name: string; readonly path: readonly ({ readonly k: "member"; readonly name: string } | { readonly k: "index"; readonly i: Expr })[]; readonly at: At };

export type Stmt =
  | { readonly k: "let" | "const" | "var"; readonly name: string; readonly type: TypeRef | undefined; readonly init: Expr | undefined; readonly at: At }
  | { readonly k: "assign"; readonly place: Place; readonly op: string; readonly value: Expr; readonly at: At }
  | { readonly k: "step"; readonly place: Place; readonly op: "++" | "--"; readonly at: At }
  | { readonly k: "if"; readonly cond: Expr; readonly then: readonly Stmt[]; readonly else: readonly Stmt[] | undefined; readonly at: At }
  | { readonly k: "for"; readonly init: Stmt | undefined; readonly cond: Expr | undefined; readonly update: Stmt | undefined; readonly body: readonly Stmt[]; readonly at: At }
  | { readonly k: "break" | "continue"; readonly at: At }
  | { readonly k: "return"; readonly value: Expr | undefined; readonly at: At }
  | { readonly k: "block"; readonly body: readonly Stmt[]; readonly at: At }
  | { readonly k: "call"; readonly call: Expr; readonly at: At };

export type Param = { readonly name: string; readonly type: TypeRef; readonly at: At };

/** Why a function is outside the subset: what, and where. */
export type RawReason = { readonly why: string; readonly at: At };

export type FnSig = { readonly name: string; readonly params: readonly Param[]; readonly ret: TypeRef | undefined; readonly at: At };

/** A function, classified. `span`/`body` are offsets into the module text (the whole definition, and its `{ ... }`). */
export type FnItem =
  | { readonly kind: "graph"; readonly sig: FnSig; readonly body: readonly Stmt[]; readonly span: readonly [number, number] }
  | { readonly kind: "raw"; readonly sig: FnSig; readonly reason: RawReason; readonly toks: readonly Tok[]; readonly span: readonly [number, number] }
  | { readonly kind: "entry"; readonly stage: "vertex" | "fragment" | "compute"; readonly sig: FnSig; readonly toks: readonly Tok[]; readonly span: readonly [number, number] };

export type StructItem = { readonly name: string; readonly fields: readonly Param[]; readonly at: At; readonly span: readonly [number, number] };
export type ConstItem = { readonly name: string; readonly type: TypeRef | undefined; readonly init: Expr; readonly at: At };
/** A module-scope declaration only raw code reads (a binding, a storage variable, `enable`, an alias), kept as text. */
export type RawItem = { readonly name: string | undefined; readonly at: At; readonly span: readonly [number, number] };

export type Module = {
  readonly fns: readonly FnItem[];
  readonly structs: readonly StructItem[];
  readonly consts: readonly ConstItem[];
  readonly raws: readonly RawItem[];
  readonly toks: readonly Tok[];
};

export class ParseError extends Error {
  constructor(readonly at: At, readonly given: string, readonly expected: string, readonly fix: string) { super(`${given}; expected ${expected}`); }
}

// ---- what puts a function outside the subset ---------------------------------------------------------------------------

const RAW_NAMES = new Map<string, string>([
  ["u32", "u32 integers"], ["i32", "i32 integers"], ["vec2u", "u32 integers"], ["vec3u", "u32 integers"], ["vec4u", "u32 integers"],
  ["vec2i", "i32 integers"], ["vec3i", "i32 integers"], ["vec4i", "i32 integers"],
  ["f16", "f16"], ["vec2h", "f16"], ["vec3h", "f16"], ["vec4h", "f16"],
  ["dpdx", "derivatives (dpdx)"], ["dpdy", "derivatives (dpdy)"], ["fwidth", "derivatives (fwidth)"],
  ["dpdxCoarse", "derivatives"], ["dpdyCoarse", "derivatives"], ["dpdxFine", "derivatives"], ["dpdyFine", "derivatives"], ["fwidthCoarse", "derivatives"], ["fwidthFine", "derivatives"],
  ["ptr", "pointers"], ["while", "a while loop"], ["loop", "a loop without a counter"], ["switch", "a switch"], ["discard", "discard"],
  ["bitcast", "bit casts"], ["workgroupBarrier", "barriers"], ["storageBarrier", "barriers"], ["textureBarrier", "barriers"], ["workgroupUniformLoad", "workgroup memory"],
  ["arrayLength", "runtime-sized arrays"], ["countOneBits", "integer bit operations"], ["countLeadingZeros", "integer bit operations"], ["countTrailingZeros", "integer bit operations"],
  ["extractBits", "integer bit operations"], ["insertBits", "integer bit operations"], ["firstLeadingBit", "integer bit operations"], ["firstTrailingBit", "integer bit operations"], ["reverseBits", "integer bit operations"],
  ["quantizeToF16", "f16"],
  // WGSL builtins the subset does not lower: raw, not an "unknown name" (the design's rule for outside the subset).
  ["ldexp", "the builtin ldexp"], ["frexp", "the builtin frexp"], ["modf", "the builtin modf"],
  ["determinant", "the builtin determinant"], ["transpose", "the builtin transpose"],
  ["dot4U8Packed", "packing"], ["dot4I8Packed", "packing"],
  ...["mat4x4", "mat2x3", "mat3x2", "mat2x4", "mat4x2", "mat3x4", "mat4x3"].flatMap((m) => [[m, `${m} matrices`], [`${m}f`, `${m} matrices`]] as [string, string][]),
  // The scene prelude's shader-stage values (src/compile/scene_wgsl.ts): raw code may read them, the CPU twin cannot.
  ["frame", "the frame uniform"], ["view_dir", "view_dir, a shader-stage value"], ["pixel_world", "pixel_world, a shader-stage value"],
  ["scene_depth", "scene_depth, a shader-stage value"], ["scene_position", "scene_position, a shader-stage value"], ["camera_ray", "camera_ray, a shader-stage value"],
]);
const RAW_PREFIXES: readonly [string, string][] = [["texture", "texture reads"], ["sampler", "texture samplers"], ["atomic", "atomics"], ["pack", "packing"], ["unpack", "packing"], ["subgroup", "subgroup operations"], ["quad", "subgroup operations"]];
const BIT_OPS = new Set(["&", "|", "^", "~", "<<", ">>", "&=", "|=", "^=", "<<=", ">>="]);

/**
 * The first construct in `toks` (a function's tokens) outside the subset, or undefined. `moduleVars` are module-scope
 * declarations only raw code reads. `i32(...)` is admitted inside a `for (...)` header, where a loop over an expression
 * count is written `i < i32(n)`.
 */
function rawReason(toks: readonly Tok[], moduleVars: ReadonlySet<string>): RawReason | undefined {
  let forDepth = -1, depth = 0;
  for (let j = 0; j < toks.length; j++) {
    const t = toks[j]!;
    const at = { line: t.line, col: t.col };
    if (t.kind === "punct") {
      if (t.s === "(") depth++;
      if (t.s === ")") { depth--; if (depth === forDepth) forDepth = -1; }
      if (BIT_OPS.has(t.s)) return { why: t.s === "&" ? "bit operations or pointers (&)" : `bit operations (${t.s})`, at };
      continue;
    }
    if (t.kind === "num") {
      if (/^0[xX]/.test(t.s) || /[iu]$/.test(t.s)) return { why: `integer literals (${t.s})`, at };
      if (/h$/.test(t.s)) return { why: `f16 literals (${t.s})`, at };
      continue;
    }
    if (t.s === "for" && toks[j + 1]?.s === "(") { forDepth = depth; continue; }
    if (t.s === "i32" && forDepth >= 0 && toks[j + 1]?.s === "(") continue;
    if (t.s === "var" && toks[j + 1]?.s === "<") return { why: "address-space variables (var<...>)", at };
    const named = RAW_NAMES.get(t.s);
    if (named !== undefined && toks[j - 1]?.s !== ".") return { why: named, at };
    if (toks[j - 1]?.s !== ".") for (const [p, why] of RAW_PREFIXES) if (t.s.startsWith(p) && t.s.length > p.length && /[A-Z_0-9]/.test(t.s[p.length]!)) return { why, at };
    if (t.s === "sampler" || t.s === "sampler_comparison") return { why: "texture samplers", at };
    if (moduleVars.has(t.s) && toks[j - 1]?.s !== ".") return { why: `the module variable ${t.s}`, at };
  }
  return undefined;
}

// ---- the parser -------------------------------------------------------------------------------------------------------

const STAGES = new Set(["vertex", "fragment", "compute"]);
const TEMPLATED = new Set(["vec2", "vec3", "vec4", "array", "mat2x2", "mat3x3", "mat4x4", "mat2x3", "mat3x2", "ptr", "atomic", "texture_2d", "texture_3d", "texture_storage_2d", "texture_storage_3d"]);
/** Binary operators by precedence, loosest first (C's order; WGSL rejects some mixes without parentheses, Loft does not). */
const LEVELS: readonly (readonly string[])[] = [["||"], ["&&"], ["|"], ["^"], ["&"], ["==", "!="], ["<", ">", "<=", ">="], ["<<", ">>"], ["+", "-"], ["*", "/", "%"]];
const ASSIGN = new Set(["=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>="]);

class Parser {
  i = 0;
  constructor(readonly toks: readonly Tok[], readonly end: number = toks.length) {}
  peek(k = 0): Tok | undefined { const j = this.i + k; return j < this.end ? this.toks[j] : undefined; }
  at(): At { const t = this.peek() ?? this.toks[this.end - 1]; return t ? { line: t.line, col: t.col } : { line: 0, col: 0 }; }
  is(s: string, k = 0): boolean { return this.peek(k)?.s === s; }
  eat(s: string): boolean { if (this.is(s)) { this.i++; return true; } return false; }
  expect(s: string, what: string): Tok {
    const t = this.peek();
    if (t?.s !== s) throw new ParseError(this.at(), t === undefined ? "the end of the text" : `"${t.s}"`, `"${s}" ${what}`, `write "${s}" here`);
    this.i++;
    return t;
  }
  ident(what: string): Tok {
    const t = this.peek();
    if (t?.kind !== "ident") throw new ParseError(this.at(), t === undefined ? "the end of the text" : `"${t.s}"`, `a name (${what})`, `write the ${what}`);
    this.i++;
    return t;
  }

  /** `@name` or `@name(...)` attributes; returns their names. */
  attributes(): string[] {
    const names: string[] = [];
    while (this.eat("@")) {
      names.push(this.ident("attribute").s);
      if (this.eat("(")) { let d = 1; while (d > 0) { const t = this.peek(); if (!t) throw new ParseError(this.at(), "the end of the text", "\")\"", "close the attribute's parentheses"); this.i++; if (t.s === "(") d++; if (t.s === ")") d--; } }
    }
    return names;
  }

  type(): TypeRef {
    const at = this.at();
    const name = this.ident("type").s;
    const args: TypeRef[] = [];
    let count: number | undefined;
    if (this.is("<") && (TEMPLATED.has(name) || /^(vec|mat|array|ptr|atomic|texture)/.test(name))) {
      this.i++;
      for (;;) {
        const t = this.peek();
        if (t?.kind === "num") { count = Number(t.s.replace(/[iu]$/, "")); this.i++; }
        else if (t?.kind === "ident" && /^(function|private|workgroup|uniform|storage|read|write|read_write)$/.test(t.s)) { args.push({ name: t.s, args: [], count: undefined, at: this.at() }); this.i++; }
        else args.push(this.type());
        if (this.eat(",")) continue;
        if (this.is(">>")) { this.splitShift(); }
        this.expect(">", "to close the type's arguments");
        break;
      }
    }
    return { name, args, count, at };
  }
  /** `>>` closing two template levels: split it into two `>` tokens in place. */
  splitShift(): void {
    const t = this.toks[this.i]!;
    const a: Tok = { ...t, s: ">", end: t.start + 1 }, b: Tok = { ...t, s: ">", start: t.start + 1, col: t.col + 1 };
    (this.toks as Tok[]).splice(this.i, 1, a, b);
  }

  params(): Param[] {
    this.expect("(", "to open the parameters");
    const out: Param[] = [];
    while (!this.is(")")) {
      this.attributes();
      const at = this.at();
      const name = this.ident("parameter").s;
      this.expect(":", "between a parameter and its type");
      out.push({ name, type: this.type(), at });
      if (!this.eat(",")) break;
    }
    this.expect(")", "to close the parameters");
    return out;
  }

  /** From `fn` (already consumed) to the opening `{` of the body. */
  signature(at: At): FnSig {
    const name = this.ident("function name").s;
    const params = this.params();
    let ret: TypeRef | undefined;
    if (this.eat("->")) { this.attributes(); ret = this.type(); }
    return { name, params, ret, at };
  }

  /** Skips a balanced `{ ... }`, returning the index after it. */
  skipBlock(): void {
    this.expect("{", "to open the body");
    let d = 1;
    while (d > 0) {
      const t = this.peek();
      if (!t) throw new ParseError(this.at(), "the end of the text", "\"}\"", "close the function's braces");
      this.i++;
      if (t.s === "{") d++;
      if (t.s === "}") d--;
    }
  }

  block(): Stmt[] {
    this.expect("{", "to open a block");
    const out: Stmt[] = [];
    while (!this.eat("}")) {
      if (this.peek() === undefined) throw new ParseError(this.at(), "the end of the text", "\"}\"", "close the block");
      out.push(this.statement());
    }
    return out;
  }

  statement(): Stmt {
    const at = this.at();
    const t = this.peek()!;
    if (t.s === "{") return { k: "block", body: this.block(), at };
    if (t.s === ";") { this.i++; return { k: "block", body: [], at }; }
    if (t.s === "let" || t.s === "const" || t.s === "var") { const s = this.declaration(); this.expect(";", "after a declaration"); return s; }
    if (t.s === "if") return this.ifStatement();
    if (t.s === "for") {
      this.i++;
      this.expect("(", "after for");
      const init = this.is(";") ? undefined : this.simple();
      this.expect(";", "after the loop's start");
      const cond = this.is(";") ? undefined : this.expr();
      this.expect(";", "after the loop's condition");
      const update = this.is(")") ? undefined : this.simple();
      this.expect(")", "to close the loop header");
      return { k: "for", init, cond, update, body: this.block(), at };
    }
    if (t.s === "break" || t.s === "continue") { this.i++; this.expect(";", `after ${t.s}`); return { k: t.s, at }; }
    if (t.s === "return") {
      this.i++;
      const value = this.is(";") ? undefined : this.expr();
      this.expect(";", "after return");
      return { k: "return", value, at };
    }
    const s = this.simple();
    this.expect(";", "after a statement");
    return s;
  }

  declaration(): Stmt {
    const at = this.at();
    const k = this.peek()!.s as "let" | "const" | "var";
    this.i++;
    const name = this.ident("name").s;
    const type = this.eat(":") ? this.type() : undefined;
    const init = this.eat("=") ? this.expr() : undefined;
    if (init === undefined && k !== "var") throw new ParseError(at, `${k} ${name} with no value`, "a value", `${k} ${name} = ...;`);
    return { k, name, type, init, at };
  }

  /** An assignment, `x++`, a declaration (in a for header) or a call. */
  simple(): Stmt {
    const at = this.at();
    if (this.is("let") || this.is("const") || this.is("var")) return this.declaration();
    const lhs = this.postfix();
    const op = this.peek()?.s;
    if (op === "++" || op === "--") { this.i++; return { k: "step", place: this.place(lhs), op, at }; }
    if (op !== undefined && ASSIGN.has(op)) { this.i++; return { k: "assign", place: this.place(lhs), op, value: this.expr(), at }; }
    if (lhs.k === "call") return { k: "call", call: lhs, at };
    throw new ParseError(at, "an expression on its own", "a statement (an assignment, a declaration, a call, if, for, return)", "assign it to something, or delete it");
  }

  place(e: Expr): Place {
    const path: Place["path"][number][] = [];
    let x = e;
    while (x.k === "member" || x.k === "index") { path.unshift(x.k === "member" ? { k: "member", name: x.name } : { k: "index", i: x.i }); x = x.a; }
    if (x.k !== "ident") throw new ParseError(e.at, "an assignment to an expression", "a variable, its field or its component", "assign to a var");
    return { name: x.name, path, at: e.at };
  }

  ifStatement(): Stmt {
    const at = this.at();
    this.expect("if", "");
    const cond = this.expr();
    const then = this.block();
    let els: Stmt[] | undefined;
    if (this.eat("else")) els = this.is("if") ? [this.ifStatement()] : this.block();
    return { k: "if", cond, then, else: els, at };
  }

  expr(level = 0): Expr {
    if (level === LEVELS.length) return this.unary();
    let a = this.expr(level + 1);
    for (;;) {
      const t = this.peek();
      if (t?.kind !== "punct" || !LEVELS[level]!.includes(t.s)) return a;
      this.i++;
      const b = this.expr(level + 1);
      a = { k: "binary", op: t.s, a, b, at: { line: t.line, col: t.col } };
    }
  }

  unary(): Expr {
    const at = this.at();
    if (this.eat("-")) return { k: "unary", op: "-", a: this.unary(), at };
    if (this.eat("!")) return { k: "unary", op: "!", a: this.unary(), at };
    if (this.is("+")) throw new ParseError(at, "a unary +", "an expression", "delete the +");
    return this.postfix();
  }

  postfix(): Expr {
    let e = this.primary();
    for (;;) {
      const at = this.at();
      if (this.eat(".")) { e = { k: "member", a: e, name: this.ident("field or swizzle").s, at }; continue; }
      if (this.eat("[")) { const i = this.expr(); this.expect("]", "to close the index"); e = { k: "index", a: e, i, at }; continue; }
      if (this.is("(")) { e = { k: "call", callee: e, type: undefined, args: this.args(), at }; continue; }
      return e;
    }
  }

  args(): Expr[] {
    this.expect("(", "to open the arguments");
    const out: Expr[] = [];
    while (!this.is(")")) { out.push(this.expr()); if (!this.eat(",")) break; }
    this.expect(")", "to close the arguments");
    return out;
  }

  primary(): Expr {
    const at = this.at();
    const t = this.peek();
    if (t === undefined) throw new ParseError(at, "the end of the text", "an expression", "finish the expression");
    if (t.kind === "num") {
      this.i++;
      const int = !/[.eEf]/.test(t.s) && !/^0[xX]/.test(t.s);
      return { k: "num", v: Number(t.s.replace(/f$/, "")), int, text: t.s, at };
    }
    if (t.s === "true" || t.s === "false") { this.i++; return { k: "bool", v: t.s === "true", at }; }
    if (t.s === "(") { this.i++; const e = this.expr(); this.expect(")", "to close the parenthesis"); return e; }
    if (t.kind === "ident") {
      // A constructor with template arguments: vec3<f32>(...), array<f32, 3>(...).
      if (this.is("<", 1) && TEMPLATED.has(t.s)) {
        const type = this.type();
        return { k: "call", callee: { k: "ident", name: type.name, at }, type, args: this.args(), at };
      }
      this.i++;
      return { k: "ident", name: t.s, at };
    }
    throw new ParseError(at, `"${t.s}"`, "an expression", "check the line for a missing operand or parenthesis");
  }
}

// ---- the module -------------------------------------------------------------------------------------------------------

/** Parse module text. Each function is classified; graph functions' bodies are parsed now, so syntax errors surface at `wgsl`. */
export function parseModule(text: string): Module {
  let toks: Tok[];
  try { toks = tokenize(text); } catch (e) {
    if (e instanceof TokenError) throw new ParseError({ line: e.line, col: e.col }, e.what, "WGSL text", "delete it");
    throw e;
  }
  // Module-scope variables first: a function that reads one is raw.
  const moduleVars = new Set<string>();
  for (let j = 0, depth = 0; j < toks.length; j++) {
    const t = toks[j]!;
    if (t.s === "{") depth++;
    else if (t.s === "}") depth--;
    else if (t.s === "var" && depth === 0) {
      let k = j + 1;
      if (toks[k]?.s === "<") { while (k < toks.length && toks[k]!.s !== ">") k++; k++; }
      const name = toks[k];
      if (name?.kind === "ident") moduleVars.add(name.s);
    }
  }
  const p = new Parser(toks);
  const fns: FnItem[] = [], structs: StructItem[] = [], consts: ConstItem[] = [], raws: RawItem[] = [];
  while (p.peek() !== undefined) {
    const first = p.i;
    const start = toks[first]!.start;
    const at = p.at();
    const attrs = p.attributes();
    const t = p.peek();
    if (t === undefined) throw new ParseError(at, "attributes with nothing after them", "a declaration", "delete them");
    if (t.s === "fn") {
      p.i++;
      const sig = p.signature(at);
      const bodyStart = p.i;
      p.skipBlock();
      const span = [start, toks[p.i - 1]!.end] as const;
      const own = toks.slice(first, p.i);
      const stage = attrs.find((a) => STAGES.has(a)) as "vertex" | "fragment" | "compute" | undefined;
      if (stage !== undefined) { fns.push({ kind: "entry", stage, sig, toks: own, span }); continue; }
      const reason = rawReason(own, moduleVars);
      if (reason !== undefined) { fns.push({ kind: "raw", sig, reason, toks: own, span }); continue; }
      const body = new Parser(toks, p.i);
      body.i = bodyStart;
      fns.push({ kind: "graph", sig, body: body.block(), span });
      continue;
    }
    if (t.s === "struct") {
      p.i++;
      const name = p.ident("struct name").s;
      p.expect("{", "to open the struct");
      const fields: Param[] = [];
      while (!p.is("}")) {
        p.attributes();
        const fat = p.at();
        const fname = p.ident("field").s;
        p.expect(":", "between a field and its type");
        fields.push({ name: fname, type: p.type(), at: fat });
        if (!p.eat(",") && !p.eat(";")) break;
      }
      p.expect("}", "to close the struct");
      p.eat(";");
      structs.push({ name, fields, at, span: [start, toks[p.i - 1]!.end] });
      continue;
    }
    if (t.s === "const") {
      const d = p.declaration();
      p.expect(";", "after a constant");
      if (d.k === "const" && d.init !== undefined) consts.push({ name: d.name, type: d.type, init: d.init, at: d.at });
      continue;
    }
    if (t.s === "override") {
      throw new ParseError(at, "an override declaration", "a hole", "bring the number in as a param through a hole: ${param(\"name\", 1.2, { min, max })}");
    }
    if (t.s === "var" || t.s === "alias" || t.s === "enable" || t.s === "requires" || t.s === "diagnostic" || t.s === "const_assert") {
      p.i++;
      if (t.s === "var" && p.is("<")) { while (!p.eat(">")) p.i++; }
      const name = p.peek()?.kind === "ident" ? p.peek()!.s : undefined;
      while (!p.eat(";")) { if (p.peek() === undefined) throw new ParseError(at, `a ${t.s} declaration with no ";"`, "\";\"", "end it with ;"); p.i++; }
      raws.push({ name: t.s === "var" || t.s === "alias" ? name : undefined, at, span: [start, toks[p.i - 1]!.end] });
      continue;
    }
    throw new ParseError(at, `"${t.s}" at module scope`, "fn, struct, const, or a binding", t.s === "let" ? "write const at module scope" : "move it inside a function");
  }
  for (const r of raws) if (r.name?.startsWith(HOLE_PREFIX)) throw new ParseError(r.at, "a hole as a declared name", "an identifier", "name it in the text");
  return { fns, structs, consts, raws, toks };
}
