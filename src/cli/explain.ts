// `loft check --explain`: why a ranged check came out as it did, in the author's lines (docs/proposals/
// 2026-11-18_authoring_api.md section 7). For a check, at its witness (the point where the search found the margin
// least, for `always`, or greatest, for `eventually`): the comparison that sets the margin there (an `and` is decided by
// its smaller side), the values of its two sides, the lines the comparison and what it reads were written on, each
// quoted from its file, and the `loft render` command that draws the scene at the witness.
//
// Lines come from the text parser (`wgsl` modules site every node for free) or, for call-form nodes, from the stack
// recorded when the node was built (`recordStacks`, turned on before the scene is imported).
import { readFileSync } from "node:fs";
import { isAbsolute, join } from "node:path";
import { Expr, stackOf } from "../expr.ts";
import { sitesOf } from "../errors.ts";
import { Evaluator, type I } from "../interval.ts";
import { siteOfNode } from "../text/index.ts";
import type { Probe } from "../check.ts";
import { heldBy, type Decision, type Quantified, type Ranged } from "../ranged.ts";

/** The library's root, which sites are relative to (src/errors.ts). */
const ROOT = new URL("../../", import.meta.url).pathname;

/** Where to render a witness: the scene file, the variant asked for (as typed), and the camera to look through. */
export type RenderAt = { readonly path: string; readonly variant: string | undefined; readonly camera: string | undefined };

/** The lines that explain one check's decision, ready to print. */
export function explain(check: Ranged, d: Decision, probe: Probe, at: RenderAt): string[] {
  const out = [`explain: ${check.name}`];
  if (check.site !== undefined) out.push(`  written at ${quote(check.site)}`);
  const verdict = d.holds === true ? "holds" : d.holds === false ? "FAIL" : "undecided";
  const witness = Object.entries(d.witness);
  if (witness.length === 0) {
    out.push(`  ${verdict}: no witness (the search found no point to evaluate: ${d.spent.kind}); nothing to explain at a point`);
    return out;
  }
  const where = witness.map(([k, v]) => `${k}=${round(v)}`).join(", ");
  out.push(`  ${verdict}: margin ${round(d.margin)} at ${where} (proven bounds [${round(d.bounds[0])}, ${round(d.bounds[1])}])`);

  // The witness as a box of points, keyed as the interval evaluator keys leaves.
  const keys = new Map<string, string>();
  for (let q: Quantified | undefined = check.formula; q; q = q.body.kind === "nested" ? q.body.q : undefined) for (const l of q.leaves) keys.set(l.label, l.key);
  const box = new Map<string, I>(witness.map(([label, v]) => [keys.get(label)!, [v, v]]));
  const ev = new Evaluator(box, heldBy(probe), check.name, { nodes: 0, codomain: 0, tableFills: probe.compiled.tableFills });
  const value = (e: Expr): number[] => ev.enclose(e).map(([lo, hi]) => lo / 2 + hi / 2);

  let atom: Quantified["body"] = check.formula.body;
  while (atom.kind === "nested") atom = atom.q.body;
  const deciding = decidingComparison(atom.condition, value);
  const [a, b] = deciding.args;
  out.push(`  decided by ${deciding.fn}(a, b)${siteText(deciding.e)}`);
  out.push(`    a = ${shownValue(value(a))}${siteText(a, deciding.e)}`);
  out.push(`    b = ${shownValue(value(b))}${siteText(b, deciding.e)}`);
  const reads = readsOf(deciding.e);
  if (reads.length > 0) {
    out.push("  it reads, at the witness:");
    for (const r of reads) out.push(`    ${shownValue(value(r.e)).padEnd(28)} ${quote(r.site)}`);
  }
  if (sitesOfNode(deciding.e).length === 0 && reads.length === 0)
    out.push("  (no line recorded for these nodes: they were built inside the library, or before the scene was imported)");
  out.push(`  see it: ${renderCommand(at, witness.map(([label, v]) => [keys.get(label)!, v]))}`);
  return out;
}

type Comparison = { readonly fn: "ge" | "gt" | "le" | "lt"; readonly e: Expr<"bool">; readonly args: readonly [Expr, Expr] };

/**
 * The comparison whose margin is the condition's at the witness: `and` takes its smaller side, `or` its greater, `not`
 * its operand (src/ranged.ts `marginOf` is the same walk; a condition `ranged` accepted has only these nodes).
 */
function decidingComparison(c: Expr<"bool">, value: (e: Expr) => number[]): Comparison {
  const n = c.node;
  if (n.op !== "call") throw new Error(`internal: explain: a condition that is a ${n.op}`);
  const [x, y] = n.args as [Expr, Expr];
  switch (n.fn) {
    case "ge": case "gt": case "le": case "lt": return { fn: n.fn, e: c, args: [x, y] };
    case "not": return decidingComparison(x as Expr<"bool">, value);
    case "and": case "or": {
      const [p, q] = [decidingComparison(x as Expr<"bool">, value), decidingComparison(y as Expr<"bool">, value)];
      const mp = marginAt(p, value), mq = marginAt(q, value);
      return (n.fn === "and") === (mp <= mq) ? p : q;
    }
    default: throw new Error(`internal: explain: a condition built by ${n.fn}`);
  }
}
const marginAt = (c: Comparison, value: (e: Expr) => number[]): number => {
  const [a, b] = c.args.map((e) => value(e)[0]!) as [number, number];
  return c.fn === "ge" || c.fn === "gt" ? a - b : b - a;
};

/** The author's lines a node was built on, innermost first: its text site, or the recorded stack's author frames. */
function sitesOfNode(e: Expr): string[] {
  const text = siteOfNode(e);
  if (text !== undefined) return [text.replace(/ .*$/, "")];
  return sitesOf(stackOf(e));
}

/** ` at file:line \`text\`` and its callers, or nothing; a site equal to `parent`'s first one is not repeated. */
function siteText(e: Expr, parent?: Expr): string {
  // A constant or a leaf is interned: its line is wherever an equal one was first built, which says nothing here.
  const op = e.node.op;
  if (op === "const" || op === "var" || op === "input" || op === "param") return "";
  const s = sitesOfNode(e);
  if (s.length === 0 || (parent !== undefined && sitesOfNode(parent)[0] === s[0])) return "";
  const [first, ...callers] = s;
  return `  at ${quote(first!)}${callers.slice(0, 3).map((c) => `\n      called from ${quote(c)}`).join("")}`;
}

/**
 * The nearest nodes under a comparison written on a line of their own (one per line, at most `limit`): what the
 * comparison reads, named where the author wrote it. Constants and leaves are skipped; they have no line of interest.
 */
function readsOf(root: Expr, limit = 6): { readonly e: Expr; readonly site: string }[] {
  const own = new Set(sitesOfNode(root).slice(0, 1));
  const seen = new Set<number>(), found: { e: Expr; site: string }[] = [];
  let level: Expr[] = [root];
  while (level.length > 0 && found.length < limit) {
    const next: Expr[] = [];
    for (const e of level) {
      for (const k of childrenOf(e)) {
        if (seen.has(k.id)) continue;
        seen.add(k.id);
        const op = k.node.op;
        if (op === "const" || op === "var" || op === "input" || op === "param") continue;
        const s = sitesOfNode(k)[0];
        if (s !== undefined && !own.has(s) && k.ty !== "bool") { own.add(s); found.push({ e: k, site: s }); if (found.length >= limit) break; }
        else next.push(k);
      }
    }
    level = next;
  }
  return found;
}

/** A node's operands, as the graph holds them (a loop's result reads its state's start values and its count). */
function childrenOf(e: Expr): readonly Expr[] {
  const n = e.node;
  switch (n.op) {
    case "lib": case "call": case "vec": return n.args;
    case "get": return [n.a];
    case "row": return [n.x];
    case "fetch": return [n.index];
    case "iterate": return [...n.loop.init, n.loop.count, ...n.loop.next];
    case "const": case "var": case "input": case "param": return [];
  }
}

/** `file:line` and the line itself, read from the file: a reader sees what was written without opening it. */
function quote(site: string): string {
  const m = site.match(/^(.*):(\d+)$/);
  if (m === null) return site;
  try {
    const line = readFileSync(isAbsolute(m[1]!) ? m[1]! : join(ROOT, m[1]!), "utf8").split("\n")[Number(m[2]) - 1]?.trim();
    return line ? `${site}  \`${line.length > 110 ? `${line.slice(0, 107)}...` : line}\`` : site;
  } catch {
    // A site whose file cannot be read (a module evaluated from a string) is still a site; print it bare.
    return site;
  }
}

/**
 * The `loft render` command that draws the witness: its time, params and inputs (by leaf key: `input:time`,
 * `param:<name>`, `input:<name>`, `var:<name>`), the variant and the scene's first camera. A free variable is no
 * setting of the scene, so the command draws the witness's other leaves and says which variable it leaves out.
 */
function renderCommand(at: RenderAt, witness: readonly (readonly [string, number])[]): string {
  const parts = ["loft render", at.path, "--out", `out/explain/${at.path.replace(/^.*\//, "").replace(/\.[^.]*$/, "")}.webp`];
  if (at.camera !== undefined) parts.push("--views", at.camera);
  if (at.variant !== undefined) parts.push("--variant", at.variant);
  const free: string[] = [];
  for (const [key, v] of witness) {
    const [kind, name] = [key.slice(0, key.indexOf(":")), key.slice(key.indexOf(":") + 1)];
    if (key === "input:time") parts.push("--time", fixed(v));
    else if (kind === "param") parts.push("--param", `${name}=${fixed(v)}`);
    else if (kind === "input") parts.push("--input", `${name}=${fixed(v)}`);
    else free.push(`${name}=${fixed(v)}`);
  }
  return `${parts.join(" ")}${free.length ? `   (the variable${free.length > 1 ? "s" : ""} ${free.join(", ")} ${free.length > 1 ? "are" : "is"} not a setting of the scene)` : ""}`;
}

const fixed = (v: number): string => String(Number(v.toFixed(4)));
const round = (v: number): string => (Number.isFinite(v) ? String(Number(v.toPrecision(5))) : String(v));
const shownValue = (v: readonly number[]): string => (v.length === 1 ? round(v[0]!) : `(${v.map(round).join(", ")})`);
