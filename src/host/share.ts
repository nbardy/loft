// Draw shaders that are one shader up to their numbers, compiled once.
//
// A scene's repeated parts (eight legs, five spikes, a left and a right claw) emit one draw shader each, and each is
// its own module and pipeline: the same WGSL with different literals (radii, control points, colours, the name's ID)
// and a different table base. Pipeline creation is most of a cold load (2026-11-15: 46 of 73 pipelines, 8 of 12 s,
// were such surfaces), so `share` groups the shaders that differ only in those numbers, compiles one module per group,
// and moves the numbers that differ into rows of data: each draw reads its row, chosen by the high bits of its
// instance index (the host adds `instanceBase` to the first instance of every run it draws).
//
// What may differ inside a group, as a "hole" read from the row:
//   - the base of a table accessor (`t_name(i)`, which reads loft_tables_k[base + i]): integer, exact;
//   - the `LOFT_DETAIL_SLOT` override, which is per draw, not per pipeline: integer, exact;
//   - a module-scope `const NAME: f32|u32|i32 = literal;` (a surface's ID), its uses then read the row;
//   - a float, u32 or i32 literal in a function only the fragment stage reaches (a paint colour), outside a
//     const-expression context, not a divisor, exponent or smoothstep edge (`folds`), and not in a function with
//     control flow (a value read through a flat varying is not uniform; WGSL rejects derivatives under a branch on it).
// Everything else, identifiers declared in the shader aside (renamed in the comparison only), must be equal.
//
// Why not the vertex stage's literals, the ones that actually differ between a left and a right leg: the row holds
// the same f32 (Math.fround of the same double Tint parses), but the shader compiler folds and reassociates constants
// it can see, so the geometry rounds differently. Measured 2026-11-15 with them shared: 7 of 8 bench programs
// identical, but one silhouette sample of train-11-group-2 moved (two pixels, 58/255, one frame of four), and 19-27
// render pipelines instead of 33-53. Kept literal, all eight programs are sha256-identical.
//
// Imports no bun-only module (the browser host uses it, through lower.ts).

import type { Artefact } from "../artefact.ts";

/** How a draw's shader is compiled and drawn after sharing. */
export type Sharing = {
  /** The key the module of `shader` is known by (shared shaders: their group's) and its WGSL. */
  readonly moduleOf: (shader: string) => Module;
  /** Draw `k`'s pipeline constants (the per-draw ones removed), what it adds to every first instance, its pipeline's cull. */
  readonly drawOf: (k: number) => { readonly constants: Readonly<Record<string, number>>; readonly instanceBase: number; readonly cull: GPUCullMode };
  /** Every group's rows, one float per hole; empty when nothing is shared (and then nothing is bound). */
  readonly rows: Float32Array;
};

/**
 * A draw shader's module: its key, its WGSL, and a directive that must open the module (WGSL directives precede every
 * declaration, so lower puts it before the prelude): "" or UNIFORM_PER_DRAW.
 */
export type Module = { readonly key: string; readonly wgsl: string; readonly directive: string };
/**
 * For a shared shader whose fragment stage branches on a row value (INK, the cull code). The analysis calls a value from
 * a flat varying non-uniform, and reports the derivatives the style's shading takes under that branch where they are
 * taken (deep in the scene WGSL, so an attribute on fs does not reach them). The value is one per draw, and a 2 x 2 quad
 * never spans two draws (it is one triangle's), so the derivatives stay defined. Every other line of such a module
 * passed the analysis unshared.
 */
export const UNIFORM_PER_DRAW = "diagnostic(off, derivative_uniformity);\n";

/** The group 0 binding of `rows` (PRELUDE_WGSL holds bindings 0 to 4). */
export const ROWS_BINDING = 5;
/** Instance bits for the copy; the row is above them. */
const COPY_BITS = 16;
/**
 * Overrides that are per draw, not per pipeline, so they become row values: the level-of-detail slot (compile/index.ts
 * sets it) and INK, which tells a surface's outline draw from its fill (draw/surface.ts hullOutline).
 */
const PER_DRAW = new Set(["LOFT_DETAIL_SLOT", "INK"]);
/** A draw's culling as a row value; a group whose draws cull differently draws with none and discards in fs. */
const CULL: Record<GPUCullMode, number> = { none: 0, back: 1, front: 2 };
/** Larger u32 values would not survive the row's f32. */
const EXACT = 1 << 24;

type Kind = "f32" | "u32" | "i32";
type Tok = { readonly text: string; readonly code: boolean };

const TOKEN = /\s+|\/\/[^\n]*|\/\*[\s\S]*?\*\/|[A-Za-z_][A-Za-z0-9_]*|0[xX][0-9a-fA-F]+[iu]?|(?:\d+\.\d*|\.\d+)(?:[eE][+-]?\d+)?f?|\d+[eE][+-]?\d+f?|\d+[iuf]?|[^]/gy;
const IDENT = /^[A-Za-z_]/;
const FLOAT = /^(?:\d+\.\d*|\.\d+|\d+[eE])/;
/** Identifiers whose `<` opens a template list (numbers inside are types' sizes, never values). */
const TEMPLATED = /^(?:array|vec[234]|mat[234]x[234]|ptr|atomic|texture_\w+|binding_array|var)$/;
const FLOW = new Set(["if", "for", "while", "loop", "switch"]);
/**
 * Numbers the shader compiler rewrites when they are constants, so a row would round differently: a divisor
 * (x / c becomes x * (1 / c)), an exponent (pow(x, 2.0) becomes x * x), smoothstep's edges (1 / (e1 - e0) folded).
 * They stay literal (a precaution: no bench pixel needed it once vertex literals stayed).
 */
function folds(prev: string, prev2: string, call: { name: string; arg: number } | undefined): boolean {
  if (prev === "/" || (prev === "-" && prev2 === "/")) return true;
  if (call === undefined) return false;
  return (call.name === "pow" && call.arg === 1) || (call.name === "smoothstep" && call.arg < 2);
}
const DECLARES = new Set(["fn", "let", "var", "const", "override", "struct", "alias"]);

function tokenize(text: string): Tok[] {
  const out: Tok[] = [];
  TOKEN.lastIndex = 0;
  for (let m = TOKEN.exec(text); m !== null && m[0].length > 0; m = TOKEN.exec(text)) {
    out.push({ text: m[0], code: !/^\s/.test(m[0]) && !m[0].startsWith("//") && !m[0].startsWith("/*") });
  }
  return out;
}

const numberKind = (t: string): Kind | undefined => {
  if (t.endsWith("u") && !t.startsWith("0x")) return "u32";
  if (t.endsWith("i") && !t.startsWith("0x")) return "i32";
  if (FLOAT.test(t) || t.endsWith("f")) return "f32";
  return undefined; // an abstract int: its type depends on its context
};
const numberValue = (t: string, kind: Kind): number => (kind === "f32" ? Math.fround(Number(t.replace(/f$/, ""))) : parseInt(t, 10));

/** A table accessor of the scene WGSL: `fn t_name(i: u32) -> vec4<f32> { return BUFFER[BASEu + i]; }`. */
type Accessor = { readonly buffer: string; readonly base: number };
const ACCESSOR = /fn (t_\w+)\(i: u32\) -> vec4<f32> \{ return (loft_\w+)\[(\d+)u \+ i\]; \}/g;
function accessors(sceneWgsl: string): Map<string, Accessor> {
  return new Map([...sceneWgsl.matchAll(ACCESSOR)].map((m) => [m[1]!, { buffer: m[2]!, base: Number(m[3]) }]));
}

/** The templates a shared shader must have (draw/surface.ts, draw/relief.ts): plumb() edits these lines. */
const VS = /@vertex fn vs\(([^\n]*)\) -> (\w+) \{/;
const INSTANCE = /@builtin\(instance_index\) (\w+): u32/;
const fsOf = (out: string) => new RegExp(`@fragment fn fs\\((\\w+): ${out}\\b[^{]*\\{`);

/** A place in one shader that may hold a different number in another member of its group. */
type Slot =
  | { readonly kind: "literal"; readonly at: number; readonly type: Kind }
  | { readonly kind: "base"; readonly at: number; readonly buffer: string }      // `at`: the accessor's identifier
  | { readonly kind: "const"; readonly at: number; readonly type: Kind; readonly name: string; readonly from: number; readonly to: number };

/** One shader, read: its tokens, the key it shares by, and its slots' values (in slot order, the same in a group). */
type Read = { readonly toks: readonly Tok[]; readonly key: string; readonly slots: readonly Slot[]; readonly values: readonly number[];
  /** The per-draw overrides it declares, with their declarations' token ranges. */
  readonly overrides: readonly Override[];
  /** The name of fs's `@builtin(front_facing)` parameter, if it has one. */
  readonly front: string | undefined };
type Override = { readonly name: string; readonly type: Kind; readonly def: number; readonly from: number; readonly to: number };

/**
 * The shader's sharing key, or why it cannot be shared. Shareable shaders are draw shaders with a `vs` taking
 * `@builtin(instance_index)` and an `fs` taking the `VOut` it returns (the surface and relief templates).
 */
function read(wgsl: string, tables: ReadonlyMap<string, Accessor>): Read | string {
  const toks = tokenize(wgsl);
  const code = toks.map((t, i) => (t.code ? i : -1)).filter((i) => i >= 0);
  const at = (k: number): string => (k >= 0 && k < code.length ? toks[code[k]!]!.text : "");
  // Pass 1: declared names, functions (with their token ranges), and where numbers are context, not values.
  const declared = new Map<string, number>();
  const fnOf: (string | undefined)[] = [];
  const fixed: boolean[] = [];
  const fns = new Map<string, { from: number; to: number; flow: boolean; calls: Set<string> }>();
  let depth = 0, fn: string | undefined, template = 0, paren = 0, attribute = -1, statement: "const" | "case" | undefined;
  // The calls the token is inside, innermost last, with the index of the argument it is in.
  const calls: { name: string; arg: number }[] = [];
  for (let k = 0; k < code.length; k++) {
    const t = at(k);
    if (DECLARES.has(t)) {
      let n = k + 1;
      if (at(n) === "<") { let d = 0; do { if (at(n) === "<") d++; if (at(n) === ">") d--; n++; } while (d > 0 && n < code.length); }
      const name = at(n);
      if (IDENT.test(name) && !declared.has(name)) declared.set(name, declared.size);
      if (t === "fn" && depth === 0) { fn = name; fns.set(name, { from: k, to: k, flow: false, calls: new Set() }); }
    }
    if ((t === "const" || t === "override") && statement === undefined) statement = "const";
    if (t === "case") statement = "case";
    if (t === "@") attribute = paren;
    if (t === "<" && TEMPLATED.test(at(k - 1))) template++;
    else if (t === ">" && template > 0) template--;
    if (t === "(") { paren++; calls.push({ name: IDENT.test(at(k - 1)) ? at(k - 1) : "", arg: 0 }); }
    if (t === ")") { paren--; calls.pop(); if (attribute === paren) attribute = -1; }
    if (t === "," && calls.length > 0) calls[calls.length - 1]!.arg++;
    fixed.push(template > 0 || statement !== undefined || attribute >= 0 || depth === 0 || fn === undefined || folds(at(k - 1), at(k - 2), calls[calls.length - 1]));
    fnOf.push(depth > 0 ? fn : undefined);
    if (fn !== undefined && depth > 0) {
      const f = fns.get(fn)!;
      if (FLOW.has(t)) f.flow = true;
      if (IDENT.test(t) && at(k + 1) === "(") f.calls.add(t);
    }
    if (t === ";" && statement === "const") statement = undefined;
    if (t === ":" && statement === "case") statement = undefined;
    if (t === "{") depth++;
    if (t === "}") { depth--; if (depth === 0 && fn !== undefined) { fns.get(fn)!.to = k; fn = undefined; } }
  }
  const vs = VS.exec(wgsl);
  if (!vs || !INSTANCE.test(vs[1]!) || !fsOf(vs[2]!).test(wgsl) || !fns.has("vs") || !fns.has("fs")) return "not the vs(instance_index) and fs(VOut) template";
  const reached = (entry: string): Set<string> => {
    const seen = new Set<string>(), stack = [entry];
    while (stack.length > 0) {
      const f = stack.pop()!;
      if (seen.has(f) || !fns.has(f)) continue;
      seen.add(f);
      stack.push(...fns.get(f)!.calls);
    }
    return seen;
  };
  // A number keeps its value (is part of the key) when the vertex stage reaches it (header: geometry), or when the
  // fragment stage reaches it in a function with control flow (header: uniformity).
  const vertex = reached("vs"), fragment = reached("fs");
  const branchy = (k: number): boolean => { const f = fnOf[k]; return f !== undefined && fragment.has(f) && fns.get(f)!.flow; };
  const guarded = (k: number): boolean => { const f = fnOf[k]; return f === undefined || vertex.has(f) || branchy(k); };

  // Module-scope `const NAME: T = NUMBER;`: holeable when every use is a value inside a function.
  const consts = new Map<string, { type: Kind; from: number; to: number; num: number }>();
  for (let k = 0; k + 6 < code.length; k++) {
    if (at(k) !== "const" || fnOf[k] !== undefined || at(k + 2) !== ":" || at(k + 4) !== "=" || at(k + 6) !== ";") continue;
    const type = at(k + 3), kind = numberKind(at(k + 5));
    if ((type === "f32" || type === "u32" || type === "i32") && kind === type) consts.set(at(k + 1), { type, from: k, to: k + 6, num: k + 5 });
  }
  for (let k = 0; k < code.length; k++) {
    const c = consts.get(at(k));
    if (c !== undefined && k !== c.from + 1 && (fixed[k] || branchy(k))) consts.delete(at(k));
  }

  const overrides: Override[] = [];
  const keyParts: string[] = [], slots: Slot[] = [], values: number[] = [];
  for (let k = 0; k < code.length; k++) {
    const t = at(k);
    const c = [...consts.values()].find((x) => x.num === k);
    if (c !== undefined) {
      keyParts.push(`#${c.type}`); slots.push({ kind: "const", at: code[k]!, type: c.type, name: at(c.from + 1), from: code[c.from]!, to: code[c.to]! });
      values.push(numberValue(t, c.type));
      continue;
    }
    const kind = /^[\d.]/.test(t) ? numberKind(t) : undefined;
    if (kind !== undefined && !fixed[k] && !guarded(k) && (kind === "f32" || numberValue(t, kind) < EXACT)) {
      keyParts.push(`#${kind}`); slots.push({ kind: "literal", at: code[k]!, type: kind }); values.push(numberValue(t, kind));
      continue;
    }
    const table = tables.get(t);
    if (table !== undefined && at(k + 1) === "(" && !fixed[k]) {
      keyParts.push(`T:${table.buffer}`); slots.push({ kind: "base", at: code[k]!, buffer: table.buffer }); values.push(table.base);
      continue;
    }
    // `override NAME: TYPE = LITERAL;`
    if (t === "override" && PER_DRAW.has(at(k + 1)) && at(k + 6) === ";" && numberKind(at(k + 5)) === at(k + 3)) {
      const type = at(k + 3) as Kind;
      overrides.push({ name: at(k + 1), type, def: numberValue(at(k + 5), type), from: code[k]!, to: code[k + 6]! });
    }
    const local = declared.get(t);
    keyParts.push(local !== undefined && !consts.has(t) ? `L${local}` : t);
  }
  const front = /@fragment fn fs\([^)]*@builtin\(front_facing\) (\w+): bool/.exec(wgsl)?.[1];
  return { toks, key: keyParts.join(" "), slots, values, overrides, front };
}

/** A group's row layout: its holes (slot indices), then its per-draw overrides, then the cull code when it has one. */
type Columns = { readonly holes: readonly number[]; readonly overrides: readonly Override[]; readonly cull: boolean };
const width = (c: Columns): number => c.holes.length + c.overrides.length + (c.cull ? 1 : 0);

/** The group's WGSL: the first member's, with each hole read from the draw's row (`loft_row`, set by both stages). */
function groupWgsl(first: Read, columns: Columns, start: number): string {
  const { holes } = columns;
  const read = (j: number, type: Kind): string => (type === "f32" ? `loft_k(${j}u)` : `${type}(loft_k(${j}u))`);
  const toks = first.toks;
  const nextCode = (i: number): number => { let n = i + 1; while (!toks[n]!.code) n++; return n; };
  const replace = new Map<number, string>(), rename = new Map<string, string>();
  const drop = (from: number, to: number) => { for (let i = from; i <= to; i++) replace.set(i, ""); };
  holes.forEach((s, j) => {
    const slot = first.slots[s]!;
    switch (slot.kind) {
      case "literal": replace.set(slot.at, read(j, slot.type)); break;
      // `t_name(i)` becomes `loft_tab_BUFFER(base, i)`: the same index arithmetic, the base from the row.
      case "base": replace.set(slot.at, `loft_tab_${slot.buffer}(${read(j, "u32")}, `); replace.set(nextCode(slot.at), ""); break;
      case "const": drop(slot.from, slot.to); rename.set(slot.name, read(j, slot.type)); break;
    }
  });
  columns.overrides.forEach((o, i) => { drop(o.from, o.to); rename.set(o.name, read(holes.length + i, o.type)); });
  const body = toks.map((t, i) => replace.get(i) ?? (t.code ? rename.get(t.text) ?? t.text : t.text)).join("");
  const buffers = [...new Set(holes.map((s) => first.slots[s]!).flatMap((s) => (s.kind === "base" ? [s.buffer] : [])))];
  return [
    `\n@group(0) @binding(${ROWS_BINDING}) var<storage, read> loft_rows: array<f32>;`,
    // This draw's row: set from the instance index by the vertex stage, from the flat varying by the fragment stage.
    "var<private> loft_row: u32;",
    "fn loft_k(j: u32) -> f32 { return loft_rows[loft_row + j]; }",
    ...buffers.map((b) => `fn loft_tab_${b}(base: u32, i: u32) -> vec4<f32> { return ${b}[base + i]; }`),
    plumb(body, start, width(columns), columns.cull ? { column: width(columns) - 1, front: first.front! } : undefined),
  ].join("\n");
}

/**
 * Carries the row: vs computes it from its instance index (high bits) and passes it on, fs reads it back. With `cull`,
 * fs discards the faces the draw's own cull mode would have culled (the pipeline culls none): the same fragments survive,
 * and a discarded fragment writes neither colour nor depth, as a culled one.
 */
function plumb(wgsl: string, start: number, width: number, cull: { column: number; front: string } | undefined): string {
  const [vsHead, params, outType] = VS.exec(wgsl)!;
  const ii = INSTANCE.exec(params!)![1]!;
  const struct = new RegExp(`struct ${outType} \\{([^}]*)\\}`).exec(wgsl)!;
  const next = Math.max(-1, ...[...struct[1]!.matchAll(/@location\((\d+)\)/g)].map((x) => Number(x[1]))) + 1;
  let out = wgsl.replace(struct[0], `struct ${outType} {${struct[1]}  @location(${next}) @interpolate(flat) loft_row: u32,\n}`);
  out = out.replace(vsHead, `@vertex fn vs(${params!.replace(INSTANCE, "@builtin(instance_index) loft_instance: u32")}) -> ${outType} {
  loft_row = ${start}u + (loft_instance >> ${COPY_BITS}u) * ${width}u;
  let ${ii} = loft_instance & ${(1 << COPY_BITS) - 1}u;`);
  // Every \`return X;\` of vs passes the row on (vs ends at the first line that closes it: the templates' layout).
  const from = out.indexOf("@vertex fn vs("), end = out.indexOf("\n}", from);
  const vs = out.slice(from, end).replace(/return (\w+);/g, (_, v: string) => `${v}.loft_row = loft_row;\n  return ${v};`);
  out = out.slice(0, from) + vs + out.slice(end);
  const fs = fsOf(outType!).exec(out)!;
  const discard = cull === undefined ? "" : `
  let loft_cull = loft_k(${cull.column}u);
  if ((loft_cull == ${CULL.back}.0 && !${cull.front}) || (loft_cull == ${CULL.front}.0 && ${cull.front})) { discard; }`;
  return out.replace(fs[0], `${fs[0]}\n  loft_row = ${fs[1]}.loft_row;${discard}`);
}

export function share(a: Artefact): Sharing {
  const tables = accessors(a.sceneWgsl);
  const used = [...new Set(a.draws.map((d) => d.shader))];
  const reads = new Map<string, Read>();
  for (const s of used) {
    const r = read(a.shaders[s]!, tables);
    // Unshareable (a reason string): compiled on its own, as before.
    if (typeof r === "string") continue;
    if (a.draws.some((d) => d.shader === s && d.instances > 1 << COPY_BITS)) continue;
    reads.set(s, r);
  }
  const byKey = new Map<string, string[]>();
  for (const [s, r] of reads) byKey.set(r.key, [...(byKey.get(r.key) ?? []), s]);

  const module = new Map<string, Module>();
  const draws = a.draws.map((d) => ({ constants: d.constants, instanceBase: 0, cull: d.cull }));
  const rows: number[] = [];
  let group = 0;
  for (const members of byKey.values()) {
    const mine = a.draws.flatMap((d, k) => (members.includes(d.shader) ? [k] : []));
    // One draw alone gains nothing; two draws of one shader (a fill and its outline) become one pipeline.
    if (mine.length < 2) continue;
    const first = reads.get(members[0]!)!;
    const columns: Columns = {
      holes: first.slots.map((_, j) => j).filter((j) => members.some((s) => reads.get(s)!.values[j] !== first.values[j])),
      overrides: first.overrides,
      cull: first.front !== undefined && new Set(mine.map((k) => a.draws[k]!.cull)).size > 1,
    };
    const start = rows.length;
    const rowOf = new Map<string, number>();
    for (const k of mine) {
      const d = a.draws[k]!, values = reads.get(d.shader)!.values;
      // A draw that does not set an override gets its declared default, as the pipeline would have.
      const row = [...columns.holes.map((j) => values[j]!), ...columns.overrides.map((o) => d.constants[o.name] ?? o.def), ...(columns.cull ? [CULL[d.cull]] : [])];
      const id = JSON.stringify(row);
      if (!rowOf.has(id)) { rowOf.set(id, rowOf.size); rows.push(...row); }
      const constants = Object.fromEntries(Object.entries(d.constants).filter(([n]) => !columns.overrides.some((o) => o.name === n)));
      draws[k] = { constants, instanceBase: rowOf.get(id)! * 2 ** COPY_BITS, cull: columns.cull ? "none" : d.cull };
    }
    if (rowOf.size > 2 ** (32 - COPY_BITS)) throw new Error(`share: ${rowOf.size} rows in one group overflow the instance index`);
    const key = `shared ${group++} (${members.join(", ")})`;
    const wgsl = groupWgsl(first, columns, start);
    const directive = columns.cull || columns.overrides.some((o) => o.name === "INK") ? UNIFORM_PER_DRAW : "";
    for (const s of members) module.set(s, { key, wgsl, directive });
  }
  return {
    moduleOf: (shader) => module.get(shader) ?? { key: shader, wgsl: a.shaders[shader]!, directive: "" },
    drawOf: (k) => draws[k]!,
    rows: new Float32Array(rows),
  };
}
