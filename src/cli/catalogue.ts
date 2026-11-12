// The catalogue of what Loft exports, read from the source at run time so it cannot go stale: every name an entry point
// exports, traced to its declaration, with its signature (or a type's fields), its doc comment, and the comment's
// "cannot" and "relatives" lists. `loft api` prints it (src/cli/api.ts); `loft context` prints it short.
//
// It reads the text, not the types: no TypeScript API is available here (typescript 7 ships a binary only), and a
// scan of `export` statements covers every form the library uses (declarations, `export *`, `export { a as b } from`,
// `export { a }` of an import, several constants in one statement). A form it does not know is an error naming the file.

import { readFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";

/** What one exported name is, where it is declared, and what its comment says. */
export type Entry = {
  readonly name: string;
  /** Declared as a type (type, interface) or a value (function, constant, class). A class is both; it counts as a value. */
  readonly kind: "value" | "type";
  /** The file that declares it, relative to the repository. */
  readonly file: string;
  /** The comment as written above the declaration, joined to one paragraph per blank line; empty when there is none. */
  readonly doc: string;
  /** The declaration as written, without its body: a function's parameters and result, a type's fields, a class's heritage. */
  readonly signature: string;
  /** For a constant that is an object of named parts (`draw`, `palette`, `hit`): each part and its comment. */
  readonly members: readonly { readonly name: string; readonly doc: string }[];
};

const ROOT = resolve(import.meta.dir, "../..");
const rel = (p: string) => relative(ROOT, p);

const DECL = /^export\s+(?:declare\s+)?(?:async\s+)?(function\*?|const|let|var|class|abstract\s+class|type|interface|enum)\s+([A-Za-z_$][\w$]*)/;

type FileScan = { readonly text: string; readonly lines: readonly string[] };
const scans = new Map<string, FileScan>();
function scan(path: string): FileScan {
  let s = scans.get(path);
  if (s === undefined) {
    const text = readFileSync(path, "utf8");
    s = { text, lines: text.split("\n") };
    scans.set(path, s);
  }
  return s;
}

/** The comment immediately above line `at`: a `/** *\/` block, or a run of `//` lines. */
function commentAbove(lines: readonly string[], at: number): string {
  let i = at - 1;
  if (i < 0) return "";
  const out: string[] = [];
  if (lines[i]!.trim().endsWith("*/")) {
    while (i >= 0 && !lines[i]!.trim().startsWith("/*")) out.unshift(lines[i--]!);
    if (i < 0) return "";
    out.unshift(lines[i]!);
    return joinComment(out.map((l) => l.trim().replace(/^\/\*\*?/, "").replace(/\*\/$/, "").replace(/^\*\s?/, "")));
  }
  while (i >= 0 && lines[i]!.trim().startsWith("//")) out.unshift(lines[i--]!.trim().replace(/^\/\/\s?/, ""));
  return joinComment(out);
}

/** Lines to paragraphs; a line starting "- " starts a new bullet inside its paragraph. */
function joinComment(lines: readonly string[]): string {
  const paras: string[] = [];
  let cur = "";
  for (const raw of lines) {
    const l = raw.trim();
    if (l === "") { if (cur !== "") paras.push(cur); cur = ""; continue; }
    // A bullet starts a line; so does an unindented line after a bullet (the list has ended); an indented one continues it.
    const afterBullet = /(^|\n)- [^\n]*$/.test(cur) && !/^\s{2,}/.test(raw);
    if (l.startsWith("- ") || afterBullet) cur += (cur === "" ? "" : "\n") + l;
    else cur += (cur === "" ? "" : " ") + l;
  }
  if (cur !== "") paras.push(cur);
  return paras.join("\n\n").trim();
}

/** Names declared on a line `export const a = ..., b = ...`: the first, and any after a comma at bracket depth 0. */
function constNames(line: string): string[] {
  const m = /^export\s+(?:const|let|var)\s+/.exec(line);
  if (!m) return [];
  const names: string[] = [];
  let depth = 0, inStr: string | null = null, start = m[0].length;
  const take = (from: number) => { const n = /^\s*([A-Za-z_$][\w$]*)\s*(?::[^=]*)?=/.exec(line.slice(from)); if (n) names.push(n[1]!); };
  take(start);
  for (let i = start; i < line.length; i++) {
    const c = line[i]!;
    if (inStr) { if (c === inStr && line[i - 1] !== "\\") inStr = null; continue; }
    if (c === '"' || c === "'" || c === "`") inStr = c;
    else if ("([{<".includes(c)) depth++;
    else if (")]}>".includes(c)) depth = Math.max(0, depth - 1);
    else if (c === "," && depth === 0) take(i + 1);
  }
  return names;
}

/** The statement starting at line `i` up to its closing `;` or `}` (re-export lists span lines). */
function statement(lines: readonly string[], i: number): { text: string; end: number } {
  let text = lines[i]!, j = i;
  while (!/[;}]\s*(from\s+["'][^"']+["'];?)?\s*$/.test(text) && j + 1 < lines.length && !/from\s+["']/.test(text)) text += " " + lines[++j]!;
  // A brace list ending in `}` with `from` on the same or next line.
  if (/^\s*export\s+(type\s+)?\{/.test(text) && !/from\s+["']/.test(text) && j + 1 < lines.length && /^\s*from\s+/.test(lines[j + 1]!)) text += " " + lines[++j]!;
  return { text, end: j };
}

const specifiers = (list: string) => list.split(",").map((s) => s.trim()).filter((s) => s !== "").map((s) => {
  const m = /^(type\s+)?([\w$]+)(?:\s+as\s+([\w$]+))?$/.exec(s);
  if (!m) throw new Error(`catalogue: cannot read export specifier "${s}"`);
  return { type: m[1] !== undefined, from: m[2]!, as: m[3] ?? m[2]! };
});

const resolveFrom = (file: string, spec: string) => resolve(dirname(file), spec);

/** Every name `file` exports, each traced to its declaration. Memoised: `export *` trees share files. */
const exportCache = new Map<string, Map<string, Entry>>();
export function exportsOf(file: string): Map<string, Entry> {
  const cached = exportCache.get(file);
  if (cached) return cached;
  const out = new Map<string, Entry>();
  exportCache.set(file, out);
  const { lines } = scan(file);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!;
    if (!line.startsWith("export")) continue;
    const d = DECL.exec(line);
    if (d) {
      const kind = d[1] === "type" || d[1] === "interface" ? "type" : "value";
      const doc = commentAbove(lines, i);
      const names = kind === "value" && /^export\s+(const|let|var)\s/.test(line) ? constNames(line) : [d[2]!];
      // An overloaded function: the comment sits above the first signature; later ones must not erase it.
      const members = names.length === 1 && /^export\s+const\s+[\w$]+(\s*:[^=]+)?\s*=\s*\{\s*$/.test(line) ? membersAfter(lines, i) : [];
      const signature = signatureAt(lines, i, kind);
      for (const name of names) if (!(doc === "" && out.get(name)?.file === rel(file))) out.set(name, { name, kind, file: rel(file), doc, signature, members });
      continue;
    }
    const { text, end } = statement(lines, i);
    i = end;
    let m: RegExpExecArray | null;
    if ((m = /^export\s+\*\s+from\s+["']([^"']+)["']/.exec(text))) {
      for (const [k, v] of exportsOf(resolveFrom(file, m[1]!))) if (k !== "default") out.set(k, v);
    } else if ((m = /^export\s+(type\s+)?\{([^}]*)\}\s*from\s+["']([^"']+)["']/.exec(text))) {
      const target = exportsOf(resolveFrom(file, m[3]!));
      for (const s of specifiers(m[2]!)) {
        const e = target.get(s.from);
        if (!e) throw new Error(`catalogue: ${rel(file)} re-exports "${s.from}", which ${m[3]} does not export`);
        out.set(s.as, { ...e, name: s.as });
      }
    } else if ((m = /^export\s+(type\s+)?\{([^}]*)\}\s*;?\s*$/.exec(text))) {
      for (const s of specifiers(m[2]!)) out.set(s.as, { ...localOrImported(file, s.from), name: s.as });
    } else if (/^export\s+default\b/.test(text)) {
      out.set("default", { name: "default", kind: "value", file: rel(file), doc: commentAbove(lines, i), signature: "", members: [] });
    } else {
      throw new Error(`catalogue: ${rel(file)}:${i + 1}: an export form this scanner does not read: ${text.slice(0, 80)}`);
    }
  }
  return out;
}

const squash = (t: string) => t.replace(/\s+/g, " ").trim();

/**
 * A declaration without its body. A type that is an object lists its fields (`name`, `name?`); any other type is its
 * text. A function is everything before the brace that opens its body; a constant, its annotation or its arrow's
 * parameters; a class, its heading. Long ones are cut at 300 characters, marked with "...".
 */
function signatureAt(lines: readonly string[], at: number, kind: "value" | "type"): string {
  const head = lines[at]!.replace(/^export\s+(declare\s+)?/, "");
  const cut = (t: string) => (t.length > 300 ? `${t.slice(0, 297)}...` : t);
  if (kind === "type") {
    if (/=\s*\{\s*$/.test(head) || /^interface\b.*\{\s*$/.test(head)) {
      const fields: string[] = [];
      for (let i = at + 1; i < lines.length && !/^\}/.test(lines[i]!); i++) {
        const m = /^ {2}(?:readonly\s+)?([A-Za-z_$][\w$]*)(\??)\s*[:(]/.exec(lines[i]!);
        if (m) fields.push(m[1]! + m[2]!);
      }
      return cut(`${squash(head.replace(/\{\s*$/, ""))} { ${fields.join(", ")} }`);
    }
    let t = head, j = at;
    while (!/;\s*$/.test(t) && j + 1 < lines.length && j < at + 8) t += " " + lines[++j]!.trim();
    return cut(squash(t.replace(/;\s*$/, "")));
  }
  if (/^(async\s+)?function|^(abstract\s+)?class/.test(head)) {
    let t = head, j = at;
    while (!/\{\s*$/.test(t) && !/;\s*$/.test(t) && j + 1 < lines.length && j < at + 12) t += " " + lines[++j]!.trim();
    return cut(squash(t.replace(/\s*\{\s*$/, "").replace(/;\s*$/, "")));
  }
  // A constant: its annotation, or an arrow's parameters and result, or the start of its value.
  const ann = /^(const|let|var)\s+([\w$]+)\s*:\s*(.+?)\s*=\s/.exec(head);
  if (ann) return cut(`const ${ann[2]}: ${ann[3]}`);
  const arrow = /^(const|let|var)\s+([\w$]+)\s*=\s*(<[^=]*>)?\s*(\(.*?\)(?:\s*:\s*[^=]+?)?)\s*=>/.exec(head);
  if (arrow) return cut(`const ${arrow[2]} = ${arrow[3] ?? ""}${arrow[4]} =>`);
  return cut(squash(head.replace(/\{\s*$/, "{ ... }").replace(/;\s*$/, "")));
}

/** The named parts of an object literal opened on line `at`: keys at two spaces' indent, up to the closing brace. */
function membersAfter(lines: readonly string[], at: number): { name: string; doc: string }[] {
  const out: { name: string; doc: string }[] = [];
  for (let i = at + 1; i < lines.length && !/^\}/.test(lines[i]!); i++) {
    const m = /^ {2}(?:async\s+)?([A-Za-z_$][\w$]*)\s*(?:[:(<]|,|$)/.exec(lines[i]!);
    if (m) out.push({ name: m[1]!, doc: commentAbove(lines, i) });
  }
  return out;
}

/** `export { a }` of something declared in the file without `export`, or imported into it. */
function localOrImported(file: string, name: string): Entry {
  const { lines, text } = scan(file);
  const imp = new RegExp(`import\\s+(?:type\\s+)?\\{[^}]*\\b${name}\\b[^}]*\\}\\s*from\\s+["']([^"']+)["']`).exec(text);
  if (imp) {
    const e = exportsOf(resolveFrom(file, imp[1]!)).get(name);
    if (e) return e;
  }
  const decl = new RegExp(`^(?:function\\*?|const|let|var|class|type|interface)\\s+${name}\\b`);
  const at = lines.findIndex((l) => decl.test(l));
  if (at < 0) throw new Error(`catalogue: ${rel(file)} exports "${name}" but its declaration was not found`);
  return { name, kind: /^(type|interface)\s/.test(lines[at]!) ? "type" : "value", file: rel(file), doc: commentAbove(lines, at), signature: signatureAt(lines, at, /^(type|interface)\s/.test(lines[at]!) ? "type" : "value"), members: [] };
}

/** The first sentence of a comment (to the first ". " before a capital, or the paragraph's end). */
export function firstSentence(doc: string): string {
  const para = doc.split("\n\n")[0]!.replace(/\n/g, " ");
  const m = /^(.+?[.!?])(?:\s+(?=[A-Z`(])|$)/.exec(para);
  return (m ? m[1]! : para).trim();
}

/**
 * The lines of a comment's list introduced by a line matching `header`: the bullets that follow it, up to the first line
 * that is not a bullet. With no bullets after it, the header's own sentence. This is how a component's comment states
 * what it cannot do and which relatives to try (`spiralTube` in src/geom/spiral.ts is the worked example).
 */
function listUnder(doc: string, header: RegExp): string[] {
  const out: string[] = [];
  const lines = doc.split("\n");
  for (let i = 0; i < lines.length; i++) {
    if (lines[i]!.startsWith("- ") || !header.test(lines[i]!)) continue;
    const items: string[] = [];
    for (let j = i + 1; j < lines.length && lines[j]!.startsWith("- "); j++) items.push(lines[j]!.slice(2).trim());
    if (items.length > 0) out.push(...items);
    else out.push(...lines[i]!.split(/(?<=[.;])\s+(?=[A-Z])/).filter((x) => header.test(x)).map((x) => x.trim()));
  }
  return out;
}

/** What the thing cannot do, as its comment states it. A limit that is not written down is read as the whole space. */
export const cannotLines = (doc: string): string[] => listUnder(doc, /\bcannot\b|\bcan't\b/i);
/** The relatives its comment invites a reader to try: other shapes from similar mathematics. */
export const relativesLines = (doc: string): string[] => listUnder(doc, /\brelatives?\b/i);

/** The first sentence of a file's opening comment: what the module is for. */
export function moduleLine(file: string): string {
  const lines = scan(resolve(ROOT, file)).lines;
  const head: string[] = [];
  for (const l of lines) { if (!l.trim().startsWith("//")) break; head.push(l.trim().replace(/^\/\/\s?/, "")); }
  return firstSentence(joinComment(head));
}

export type Group = { readonly file: string; readonly line: string; readonly layer: Layer; readonly values: readonly Entry[]; readonly types: readonly Entry[] };

/**
 * The layers, bottom up. A module's layer is a judgement, so it is written here by path; a module this list does not
 * place prints under "Not yet placed", so a new module is never lost, only unsorted.
 */
export const LAYERS = [
  "Expressions and curves", "Maps", "Drawables", "Shapes", "Motion", "Actions, checks and designs", "Styles and light",
  "Scene and compiler", "Library functions (import from src/lib/index.ts)", "Not yet placed",
] as const;
export type Layer = (typeof LAYERS)[number];
const MOTION = ["lively", "motion", "gait", "stance", "ring", "placing", "grab", "bend", "pendulum", "restless", "expression", "struck"];
export function layerOf(file: string): Layer {
  const f = file.replace(/^src\//, "");
  if (/^(expr|curve|grad)\.ts$/.test(f)) return "Expressions and curves";
  if (f.startsWith("map/")) return "Maps";
  if (/^(draw|soft|relief)\//.test(f)) return "Drawables";
  if (f.startsWith("geom/")) return MOTION.includes(f.slice(5, -3)) ? "Motion" : "Shapes";
  if (/^spacetime\/|^(check|design|lineup)\.ts$/.test(f)) return "Actions, checks and designs";
  if (/^style\/|^(style|sun)\.ts$/.test(f)) return "Styles and light";
  if (/^compile\/|^(scene|artefact)\.ts$/.test(f)) return "Scene and compiler";
  if (f.startsWith("lib/")) return "Library functions (import from src/lib/index.ts)";
  return "Not yet placed";
}

/** The catalogue of one entry point: its names grouped by declaring file, the files ordered by layer, then as the entry point first names them. */
export function catalogue(entry: string): Group[] {
  const byFile = new Map<string, Entry[]>();
  for (const e of exportsOf(resolve(ROOT, entry)).values()) {
    const list = byFile.get(e.file) ?? [];
    list.push(e);
    byFile.set(e.file, list);
  }
  const groups = [...byFile].map(([file, es]): Group => ({
    file, line: moduleLine(file), layer: layerOf(file),
    values: es.filter((e) => e.kind === "value"), types: es.filter((e) => e.kind === "type"),
  }));
  // Stable: within a layer, the entry point's own order.
  return groups.map((g, k) => ({ g, k })).sort((a, b) => LAYERS.indexOf(a.g.layer) - LAYERS.indexOf(b.g.layer) || a.k - b.k).map((x) => x.g);
}
