// `wgsl`: WGSL text as a module of functions of expressions (docs/proposals/2026-11-18_authoring_api.md section 2).
//
//   const eel = wgsl`fn girth(m: f32) -> f32 { return 0.24 * sqrt(4.0 * m * (1.0 - m)) + 0.02; }`;
//   strand({ ..., girth: eel.girth });
//
// The text is parsed once, here. Each call of a module function lowers its body with the actual arguments, so a
// function in text is the same graph as the call form that builds it. A function outside the subset (src/text/parse.ts
// says which and why) is a raw function: its text is emitted as a GPU-only library function, with holes, `time` and
// this module's other functions reached through extra parameters the compiler adds.

import * as X from "../expr.ts";
import { siteOf } from "../errors.ts";
import { emitWgsl, everything, lib, markRaw, type Expr, type LibFn, type Ty } from "../expr.ts";
import { parseModule, ParseError, type At, type FnItem, type Module, type StructItem, type TypeRef } from "./parse.ts";
import { HOLE_PREFIX, type Tok } from "./tokens.ts";
import { callArg, fromTs, Lowerer, placeOf, TextError, toTs, typeName, type Scope0, type Source, type Value } from "./lower.ts";
import { PRELUDE } from "./prelude.ts";

export { siteOfNode } from "./lower.ts";
export { PRELUDE } from "./prelude.ts";

/** What a module keeps for the compiler (raw drawables and compute passes read its entry points and module text). */
export type WgslState = {
  readonly text: string;
  readonly source: Source;
  readonly module: Module;
  /** The values of the holes, in order. */
  readonly holes: readonly Value[];
  /** A raw function's library function and the expressions its added parameters take (src/text/index.ts `rawOf`). */
  raw(name: string): RawFn;
};
export const WGSL_STATE = Symbol("wgsl module");

/**
 * A WGSL module: one callable per `fn` (an ordinary function of expressions: numbers, arrays and expressions in, an
 * expression or a record of them out) and one per `struct` (its fields in order, a record out). TypeScript cannot see
 * inside the text: a wrong name or type is a SceneError at the call, quoting the text's line.
 */
export type WgslModule = { readonly [name: string]: (...args: any[]) => any } & { readonly [WGSL_STATE]: WgslState };

/**
 * WGSL text as a module of functions of expressions. As a tag, `${value}` brings in a Loft value the text has no name
 * for (a param, an input, a solved curve, a function of expressions, a number): a number or array becomes a constant,
 * a function or record is callable or readable in the text. As a call, `wgsl(text)` reads text kept in a file
 * (`import source from "./eel.wgsl" with { type: "text" }`). The subset, holes and raw functions: docs/guide.md.
 */
export function wgsl(strings: TemplateStringsArray | string, ...holes: unknown[]): WgslModule {
  const stack = new Error().stack;
  const call = siteOf(stack) ?? "an unknown site";
  const tagged = typeof strings !== "string";
  const parts = tagged ? (strings as TemplateStringsArray).raw : [strings as string];
  if (!tagged && holes.length > 0) throw new TextError(stubSource(call), { line: 0, col: 0 }, `wgsl(text) with ${holes.length} more argument(s)`, "the text alone", "use the tag form for holes: wgsl`...${x}...`");
  const [file, line] = tagged ? splitSite(call) : [undefined, 1];
  const text = parts.map((s, k) => (k < parts.length - 1 ? `${s}${HOLE_PREFIX}${k}` : s)).join("");
  const source: Source = { file, line0: line, call, lines: text.split("\n") };
  parts.forEach((s) => {
    const m = /\b(loft_\w*|__\w*)/.exec(s);
    if (m) {
      const before = text.slice(0, text.indexOf(s) + m.index).split("\n");
      throw new TextError(source, { line: before.length - 1, col: before[before.length - 1]!.length }, `the name ${m[1]}`, "a name that does not start with loft_ or __ (the compiler's own names)", "rename it");
    }
  });
  let module: Module;
  try { module = parseModule(text); } catch (e) {
    if (e instanceof ParseError) throw new TextError(source, e.at, e.given, e.expected, e.fix);
    throw e;
  }
  return assemble(text, source, module, holes);
}

function stubSource(call: string): Source { return { file: undefined, line0: 1, call, lines: [""] }; }
function splitSite(site: string): [string | undefined, number] {
  const m = /^(.*):(\d+)$/.exec(site);
  return m ? [m[1], Number(m[2])] : [undefined, 1];
}

/** A raw function's library function, and the expressions its added parameters take, in order. */
export type RawFn = { readonly lib: LibFn; readonly hidden: readonly Expr[]; readonly params: readonly TypeRef[]; readonly ret: Ty | undefined };

/** Library functions made from text, by name: the same text twice is one function (a name is its content's hash). */
const made = new Map<string, LibFn>();
function libOf(base: string, body: string, ret: Ty, deps: readonly LibFn[]): LibFn {
  const name = `${base}_${hash(body)}`;
  const wgsl = body.replaceAll(SELF, name);
  const seen = made.get(name);
  if (seen !== undefined && seen.wgsl === wgsl) return seen;
  const fn: LibFn = { name, ret, wgsl, js: "gpu-only", deps };
  made.set(name, fn);
  return fn;
}
const SELF = "\u0000self\u0000";
function hash(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) { h ^= text.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0; }
  return h.toString(16).padStart(8, "0");
}

const TY: Readonly<Record<string, Ty>> = { f32: "f32", vec2f: "vec2", vec3f: "vec3", vec4f: "vec4", bool: "bool" };
/** A WGSL type as a graph type, or undefined (u32, a struct, ...). */
function graphTy(t: TypeRef | undefined): Ty | undefined {
  if (t === undefined) return undefined;
  if (/^vec[234]$/.test(t.name) && t.args[0]?.name === "f32") return `vec${t.name[3]}` as Ty;
  return TY[t.name];
}

function assemble(text: string, source: Source, module: Module, rawHoles: readonly unknown[]): WgslModule {
  const fns = new Map<string, FnItem>(), structs = new Map<string, StructItem>(), consts = new Map(module.consts.map((c) => [c.name, c] as const));
  const names = new Map<string, At>();
  const declare = (name: string, at: At): void => {
    if (/^(f32|f16|i32|u32|bool|array|vec[234][fhiu]?|mat[234]x[234][fh]?)$/.test(name)) throw new TextError(source, at, `a declaration named ${name}`, "a name that is not one of WGSL's types", "rename it");
    const seen = names.get(name);
    if (seen !== undefined) throw new TextError(source, at, `a second declaration of ${name}`, "one declaration per name (the first is at line " + (source.line0 + seen.line) + ")", "rename one of them");
    names.set(name, at);
  };
  for (const f of module.fns) { declare(f.sig.name, f.sig.at); fns.set(f.sig.name, f); }
  for (const s of module.structs) { declare(s.name, s.at); structs.set(s.name, s); }
  for (const c of module.consts) declare(c.name, c.at);
  const failAt = (at: At) => (given: string, expected: string, fix: string): never => { throw new TextError(source, at, given, expected, fix); };
  const holes = rawHoles.map((h, k) => fromTs(h, `the hole \${...} number ${k + 1}`, failAt(holeAt(module, k))));

  const raws = new Map<string, RawFn>();
  const standalones = new Map<string, RawFn>();
  const structLibs = new Map<string, LibFn>();
  const constValues = new Map<string, Value | "busy">();
  const scope: Scope0 = {
    src: source, fns, structs, consts,
    constValue: (name) => {
      const seen = constValues.get(name);
      if (seen === "busy") return fail(consts.get(name)!.at, `${name}, defined in terms of itself`, "constants in terms of earlier ones", "reorder or rewrite the constants");
      if (seen !== undefined) return seen;
      constValues.set(name, "busy");
      const v = lowerer.constant(consts.get(name)!);
      constValues.set(name, v);
      return v;
    },
    hole: (k) => holes[k]!,
    callRaw: (name, args, at) => callRaw(name, args, at),
    prelude: (name) => (PRELUDE.has(name) ? fromTs(PRELUDE.get(name), name, failAt({ line: 0, col: 0 })) : undefined),
    names: () => [...names.keys(), ...PRELUDE.keys()],
  };
  const lowerer = new Lowerer(scope);
  const fail = (at: At, given: string, expected: string, fix: string): never => { throw new TextError(source, at, given, expected, fix); };

  /** A graph call of a raw function: a GPU-only library call, the hidden parameters appended. */
  function callRaw(name: string, args: readonly Value[], at: At): Value {
    const r = rawOf(name, []);
    const sig = fns.get(name)!.sig;
    if (r.ret === undefined) fail(at, `a call of the raw function ${name}, which returns ${sig.ret?.name ?? "nothing"}`, "a raw function returning f32, a vector or bool, for graph code to call", "return an f32 or vector (convert integers with f32(...) inside it)");
    if (args.length !== sig.params.length) fail(at, `${args.length} arguments to ${name}`, `${sig.params.length}`, `call ${name}(${sig.params.map((p) => p.name).join(", ")})`);
    const xs = args.map((a, i) => {
      const want = graphTy(sig.params[i]!.type);
      const x = callArg(a);
      const got = x === undefined ? undefined : X.lift(x as X.In);
      if (want === undefined) return fail(at, `${name}'s parameter ${sig.params[i]!.name}, a ${sig.params[i]!.type.name}`, "f32, vectors and bools from graph code", `take an f32 and convert inside ${name}`);
      if (got === undefined || got.ty !== want) return fail(at, `${name}'s argument ${i + 1} of ${typeName(a)}`, want, `pass a ${want}`);
      return got;
    });
    return { k: "expr", e: lib(r.lib, ...xs, ...r.hidden) };
  }

  function structLib(name: string): LibFn {
    const hit = structLibs.get(name);
    if (hit !== undefined) return hit;
    const st = structs.get(name)!;
    const deps: LibFn[] = [];
    const body = rewrite(st.span, (t, i, toks) => {
      if (t.kind !== "ident" || toks[i - 1]?.s === ".") return undefined;
      if (t.s === name && toks[i - 1]?.s === "struct") return SELF;
      if (structs.has(t.s) && toks[i + 1]?.s !== ":") { const d = structLib(t.s); deps.push(d); return d.name; }
      return undefined;
    });
    const made = libOf(name, body, "f32", deps);
    structLibs.set(name, made);
    return made;
  }

  /** The text of `span` with tokens replaced (undefined keeps a token), whitespace and comments kept. */
  function rewrite(span: readonly [number, number], f: (t: Tok, i: number, toks: readonly Tok[]) => string | undefined, insert?: Map<number, string>): string {
    const toks = module.toks.filter((t) => t.start >= span[0] && t.end <= span[1]);
    let out = "", at = span[0];
    toks.forEach((t, i) => {
      out += text.slice(at, t.start);
      const before = insert?.get(t.start);
      if (before !== undefined) out += before;
      out += f(t, i, toks) ?? t.s;
      at = t.end;
    });
    return out + text.slice(at, span[1]);
  }

  /**
   * A raw function as a library function. Pass one collects what it reads from outside the text (holes, `time`, the
   * hidden parameters of the functions it calls); pass two writes the text with each replaced: constants as literals,
   * expressions as added parameters `loft_h<k>`, this module's functions and structs by their library names.
   */
  function rawOf(name: string, stack: readonly string[]): RawFn {
    const hit = raws.get(name);
    if (hit !== undefined) return hit;
    const item = fns.get(name)!;
    if (item.kind !== "raw") throw new Error(`internal: text: ${name} is not raw`);
    if (stack.includes(name)) fail(item.sig.at, `${name} calling itself`, "no recursion (WGSL has none)", "write a loop");
    const toks = item.toks;
    const hidden: Expr[] = [];
    const hide = (e: Expr): string => { let k = hidden.indexOf(e); if (k < 0) k = hidden.push(e) - 1; return `loft_h${k}`; };
    const deps: LibFn[] = [];
    const locals = new Set([...item.sig.params.map((p) => p.name), ...toks.filter((t, i) => (toks[i - 1]?.s === "let" || toks[i - 1]?.s === "var" || toks[i - 1]?.s === "const") && t.kind === "ident").map((t) => t.s)]);
    const literal = (v: Value, at: At, what: string): string => {
      switch (v.k) {
        case "num": { const t = /[.e]/.test(String(v.v)) ? String(v.v) : `${v.v}.0`; return v.v < 0 ? `(${t})` : t; }
        case "lit": return `vec${v.v.length}<f32>(${v.v.map((x) => literal({ k: "num", v: x, int: false }, at, what)).join(", ")})`;
        case "bool": return String(v.b);
        case "expr": return hide(v.e);
        case "int": return typeof v.v === "number" ? `${v.v}` : hide(v.v);
        default: return fail(at, `${what}, ${typeName(v)}, read by the raw function ${name}`, "a number, a vector, a bool or an expression", "call it from a graph function and pass raw code the result");
      }
    };
    // Calls of this module's functions: the callee's added parameters go at the end of the call's arguments.
    const close = new Map<number, string>();
    const callee = (i: number, target: RawFn): void => {
      let d = 0, j = i + 1;
      for (; j < toks.length; j++) { if (toks[j]!.s === "(") d++; if (toks[j]!.s === ")") { d--; if (d === 0) break; } }
      if (target.hidden.length === 0) return;
      const names = target.hidden.map(hide);
      close.set(toks[j]!.start, `${toks[j - 1]!.s === "(" ? "" : ", "}${names.join(", ")}`);
    };
    const named = new Map<number, string>();
    toks.forEach((t, i) => {
      if (t.kind !== "ident" || toks[i - 1]?.s === "." || toks[i - 1]?.s === "@") return;
      const at = { line: t.line, col: t.col };
      const calls = toks[i + 1]?.s === "(";
      if (i > 0 && toks[i - 1]!.s === "fn") { named.set(i, SELF); return; }
      if (t.s.startsWith(HOLE_PREFIX)) { named.set(i, literal(holes[Number(t.s.slice(HOLE_PREFIX.length))]!, at, "a hole")); return; }
      if (locals.has(t.s)) return;
      if (t.s === "time") { named.set(i, hide(X.time)); return; }
      const c = consts.get(t.s);
      if (c !== undefined) { named.set(i, literal(scope.constValue(t.s), at, t.s)); return; }
      const f = fns.get(t.s);
      if (f !== undefined && calls) {
        const target = f.kind === "raw" ? rawOf(t.s, [...stack, name]) : f.kind === "graph" ? standalone(t.s) : fail(at, `a call of the entry point ${t.s}`, "a function", "call a plain fn");
        deps.push(target.lib);
        named.set(i, target.lib.name);
        callee(i, target);
        return;
      }
      if (structs.has(t.s)) { const d = structLib(t.s); deps.push(d); named.set(i, d.name); return; }
      if (calls && PRELUDE.has(t.s) && !WGSL_BUILTINS.has(t.s)) fail(at, `a call of the prelude function ${t.s} from the raw function ${name}`, "WGSL builtins, this module's functions and holes", `wrap it in a graph function of this module, fn ${t.s}_of(p: vec3f) -> f32 { return ${t.s}(p); }, and call that`);
    });
    // The added parameters, after the declared ones.
    const sigOpen = toks.findIndex((t, i) => t.s === "(" && toks[i - 1]?.kind === "ident" && toks[i - 2]?.s === "fn");
    let d = 0, sigClose = sigOpen;
    for (; sigClose < toks.length; sigClose++) { if (toks[sigClose]!.s === "(") d++; if (toks[sigClose]!.s === ")") { d--; if (d === 0) break; } }
    const finish = (): string => {
      const extra = hidden.map((e, k) => `loft_h${k}: ${X.WGSL_TY[e.ty]}`).join(", ");
      const insert = new Map(close);
      if (extra !== "") insert.set(toks[sigClose]!.start, `${(close.get(toks[sigClose]!.start) ?? "")}${toks[sigClose - 1]!.s === "(" || toks[sigClose - 1]!.s === "," ? "" : ", "}${extra}`);
      return rewrite(item.span, (_t, i) => named.get(i), insert);
    };
    const body = finish();
    const r: RawFn = { lib: libOf(name, body, graphTy(item.sig.ret) ?? "f32", deps), hidden, params: item.sig.params.map((p) => p.type), ret: graphTy(item.sig.ret) };
    markRaw(r.lib, `raw function ${name} at ${placeOf(source, item.reason.at)}: ${item.reason.why}`);
    raws.set(name, r);
    return r;
  }

  /** A graph function emitted on its own, for raw code to call: its body over variables of its declared types. */
  function standalone(name: string): RawFn {
    const hit = standalones.get(name);
    if (hit !== undefined) return hit;
    const item = fns.get(name)! as FnItem & { kind: "graph" };
    const tys = item.sig.params.map((p) => graphTy(p.type));
    const ret = graphTy(item.sig.ret);
    if (ret === undefined || tys.some((t) => t === undefined)) fail(item.sig.at, `${name}, called from raw code, with a parameter or result that is not f32, a vector or bool`, "f32, vectors and bools", "raw code can call graph functions of numbers and vectors");
    const vars = tys.map((t, k) => X.variable(`loft_a${k}`, t!));
    const out = lowerer.call(item, vars.map((e) => ({ k: "expr", e }) as Value), item.sig.at, [], undefined);
    const root = X.lift(callArg(out) as X.In);
    const hidden: Expr[] = [];
    const own = new Set(vars.map((v) => v.id));
    for (const e of everything([root])) {
      const n = e.node;
      if (n.op === "input" || n.op === "param") hidden.push(e);
      if (n.op === "var" && !own.has(e.id) && !n.name.startsWith("_L")) fail(item.sig.at, `${name}, which reads the free variable ${n.name}`, "a function of its parameters", "pass it as a parameter");
      if (n.op === "row" || n.op === "fetch") fail(item.sig.at, `${name}, which reads a curve or table, called from raw code`, "a function of its parameters, holes and time", "call it from graph code and pass raw code the result");
    }
    const hiddenName = (e: Expr): string => `loft_h${hidden.indexOf(e)}`;
    const byInput = new Map(hidden.map((e) => [e.node.op === "input" ? `i:${e.node.spec.name}` : `p:${(e.node as { spec: { name: string } }).spec.name}`, e]));
    const emitted = emitWgsl([root], {
      variable: (v) => v,
      input: (spec) => hiddenName(byInput.get(`i:${spec.name}`)!),
      param: (spec) => hiddenName(byInput.get(`p:${spec.name}`)!),
      row: () => { throw new Error("internal: text: a row in a standalone function"); },
      table: () => { throw new Error("internal: text: a table in a standalone function"); },
    });
    const params = [...vars.map((v, k) => `loft_a${k}: ${X.WGSL_TY[v.ty]}`), ...hidden.map((e, k) => `loft_h${k}: ${X.WGSL_TY[e.ty]}`)];
    const body = `fn ${SELF}(${params.join(", ")}) -> ${X.WGSL_TY[ret!]} {\n${emitted.lines.map((l) => `  ${l}\n`).join("")}  return ${emitted.out[0]};\n}`;
    const r: RawFn = { lib: libOf(name, body, ret!, emitted.libs), hidden, params: item.sig.params.map((p) => p.type), ret };
    standalones.set(name, r);
    return r;
  }

  // Module constants are lowered now, once, in order: as a TypeScript module's constants are computed when it loads.
  for (const c of module.consts) scope.constValue(c.name);

  const state: WgslState = { text, source, module, holes, raw: (name) => rawOf(name, []) };
  const out: Record<string | symbol, unknown> = { [WGSL_STATE]: state };
  const withCaller = <T>(f: () => T): T => {
    try { return f(); } catch (e) {
      // The TypeScript line that called into the text, for errors that depend on its arguments.
      if (e instanceof TextError && e.caller === undefined) { const c = siteOf(new Error().stack); if (c !== undefined) e.message += `\n  (called from ${c})`; }
      throw e;
    }
  };
  const tsFail = (name: string) => (given: string, expected: string, fix: string): never => { throw new TextError(source, fns.get(name)?.sig.at ?? { line: 0, col: 0 }, given, expected, fix); };
  for (const f of module.fns) {
    const name = f.sig.name;
    if (f.kind === "entry") continue;
    out[name] = (...args: unknown[]) => withCaller(() => {
      const n = f.sig.params.length;
      // A slot may pass more arguments than a function declares: it takes a prefix, as a JavaScript function does.
      if (args.length < n) tsFail(name)(`${args.length} argument${args.length === 1 ? "" : "s"} to ${name}`, `${n} (${f.sig.params.map((p) => p.name).join(", ")})`, `pass ${f.sig.params.map((p) => p.name).join(", ")}`);
      const values = args.slice(0, n).map((a, i) => fromTs(a, `${name}'s argument ${f.sig.params[i]!.name}`, tsFail(name)));
      const v = f.kind === "graph" ? lowerer.call(f, values, f.sig.at, [], undefined) : callRaw(name, values, f.sig.at);
      return toTs(v, name, tsFail(name));
    });
  }
  for (const s of module.structs) {
    out[s.name] = (...args: unknown[]) => withCaller(() => toTs(lowerer.structOf(s.name, s.fields, args.map((a, i) => fromTs(a, `${s.name}.${s.fields[i]?.name ?? i}`, tsFail(s.name))), s.at), s.name, tsFail(s.name)));
  }
  return out as WgslModule;
}

/** Where hole k sits in the text. */
function holeAt(module: Module, k: number): At {
  const t = module.toks.find((t) => t.s === `${HOLE_PREFIX}${k}`);
  return t ? { line: t.line, col: t.col } : { line: 0, col: 0 };
}

/** WGSL's own function names, which raw code calls as WGSL means them even where the prelude has a function of that name. */
const WGSL_BUILTINS = new Set(["abs", "sign", "floor", "ceil", "fract", "trunc", "round", "sqrt", "inverseSqrt", "exp", "exp2", "log", "log2", "pow", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "min", "max", "clamp", "saturate", "mix", "step", "smoothstep", "length", "distance", "dot", "cross", "normalize", "reflect", "refract", "faceForward", "fma", "select", "radians", "degrees", "all", "any"]);
